//! Native transport: the realtime socket over `tokio-tungstenite` with `TCP_NODELAY` and a split reader/writer. (`ewebsock` is
//! deliberately not used natively: its 10 ms read loop and missing NODELAY would be measured as "Nakama is slow".)
//! The session token travels in the URL query, so no URL is ever printed or logged.

use std::collections::VecDeque;
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::protocol::socket::Envelope;

type Ws = WebSocketStream<TcpStream>;

pub struct Socket {
    sink: SplitSink<Ws, Message>,
    stream: SplitStream<Ws>,
    next_cid: u32,
    /// Messages that arrived while waiting for a specific reply (presence events, data, ...), oldest first.
    pub events: VecDeque<Envelope>,
}

/// `http://host:7350` -> `host:7350`; only plain `http://` (stage 1) is supported natively.
fn host_port(base: &str) -> Result<&str, String> {
    base.strip_prefix("http://")
        .map(|r| r.trim_end_matches('/'))
        .ok_or_else(|| format!("only http:// endpoints are supported natively in stage 1, got a {}-scheme URL", base.split(':').next().unwrap_or("?")))
}

impl Socket {
    pub async fn connect(base_url: &str, token: &str) -> Result<Socket, String> {
        let hp = host_port(base_url)?;
        let tcp = TcpStream::connect(hp).await.map_err(|e| format!("tcp connect: {e}"))?;
        tcp.set_nodelay(true).map_err(|e| format!("set_nodelay: {e}"))?;
        let url = format!("ws://{hp}/ws?token={token}&format=json");
        let request = url.into_client_request().map_err(|e| format!("bad socket request: {e}"))?;
        let (ws, _) = tokio_tungstenite::client_async(request, tcp).await.map_err(|e| format!("websocket handshake: {e}"))?;
        let (sink, stream) = ws.split();
        Ok(Socket { sink, stream, next_cid: 1, events: VecDeque::new() })
    }

    pub async fn send(&mut self, env: &Envelope) -> Result<(), String> {
        self.send_text(env.to_json()).await
    }

    pub async fn send_text(&mut self, text: String) -> Result<(), String> {
        self.sink.send(Message::text(text)).await.map_err(|e| format!("send: {e}"))
    }

    /// The next envelope, or `Ok(None)` when nothing arrives within `wait`. `Err` when the socket is closed or broken.
    pub async fn recv(&mut self, wait: Duration) -> Result<Option<Envelope>, String> {
        loop {
            match timeout(wait, self.stream.next()).await {
                Err(_) => return Ok(None),
                Ok(None) => return Err("socket closed by the peer".into()),
                Ok(Some(Err(e))) => return Err(format!("socket error: {e}")),
                Ok(Some(Ok(Message::Text(t)))) => return Envelope::parse(t.as_str()).map(Some),
                Ok(Some(Ok(Message::Close(frame)))) => {
                    return Err(format!("socket closed: {}", frame.map(|f| format!("{} {}", u16::from(f.code), f.reason)).unwrap_or_default()))
                }
                Ok(Some(Ok(_))) => continue, // ping/pong/binary frames are not part of the JSON envelope
            }
        }
    }

    /// Send a request (a `cid` is assigned) and wait for the reply carrying the same `cid`. Other messages are kept in `events`.
    /// A server `error` reply is returned as `Err("code: message")`.
    pub async fn request(&mut self, mut env: Envelope, wait: Duration) -> Result<Envelope, String> {
        let cid = self.next_cid.to_string();
        self.next_cid += 1;
        env.cid = Some(cid.clone());
        self.send(&env).await?;
        loop {
            match self.recv(wait).await? {
                None => return Err("no reply within the timeout".into()),
                Some(reply) if reply.cid.as_deref() == Some(cid.as_str()) => {
                    return match reply.error {
                        Some(e) => Err(format!("{}: {}", e.code, e.message)),
                        None => Ok(reply),
                    }
                }
                Some(other) => self.events.push_back(other),
            }
        }
    }

    /// The first buffered or incoming envelope accepted by `pick`; non-matching ones stay in `events`.
    pub async fn wait_for<T>(&mut self, wait: Duration, mut pick: impl FnMut(&Envelope) -> Option<T>) -> Result<Option<T>, String> {
        if let Some(i) = self.events.iter().position(|e| pick(e).is_some()) {
            let env = self.events.remove(i).expect("index just found");
            return Ok(pick(&env));
        }
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            match self.recv(left).await? {
                None => return Ok(None),
                Some(env) => match pick(&env) {
                    Some(v) => return Ok(Some(v)),
                    None => self.events.push_back(env),
                },
            }
        }
    }

    pub async fn close(mut self) {
        let _ = self.sink.close().await;
    }

    /// Hand the two halves to separate tasks (E5: a writer task on a deadline schedule, a reader task that timestamps each receive
    /// the moment it arrives). Buffered `events` are dropped: split after setup is done.
    pub fn split(self) -> (Writer, Reader) {
        (Writer { sink: self.sink }, Reader { stream: self.stream })
    }
}

pub struct Writer {
    sink: SplitSink<Ws, Message>,
}

impl Writer {
    pub async fn send_text(&mut self, text: String) -> Result<(), String> {
        self.sink.send(Message::text(text)).await.map_err(|e| format!("send: {e}"))
    }
}

pub struct Reader {
    stream: SplitStream<Ws>,
}

impl Reader {
    /// The next envelope with the instant it was taken off the socket. `Err` when the socket closes or breaks.
    pub async fn next(&mut self) -> Result<(std::time::Instant, Envelope), String> {
        loop {
            match self.stream.next().await {
                None => return Err("socket closed by the peer".into()),
                Some(Err(e)) => return Err(format!("socket error: {e}")),
                Some(Ok(Message::Text(t))) => {
                    let at = std::time::Instant::now();
                    return Envelope::parse(t.as_str()).map(|e| (at, e));
                }
                Some(Ok(Message::Close(frame))) => {
                    return Err(format!("socket closed: {}", frame.map(|f| format!("{} {}", u16::from(f.code), f.reason)).unwrap_or_default()))
                }
                Some(Ok(_)) => continue,
            }
        }
    }
}

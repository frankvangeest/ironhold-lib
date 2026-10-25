//! E8 / S8: JSON envelope against protobuf envelope on the same relay. Two clients, A -> B (targeted, op 2) and B echoes to A
//! (op 3), strictly one probe in flight, `n` round trips per payload size, wire size of one data message per format.
//!   nakama_spike e8 [n=300]
//! A functional/size comparison, not the E5 load test: no load, no deadline schedule, round trips timed at the sender.

use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use nakama_spike::config::Config;
use nakama_spike::native::Socket;
use nakama_spike::protocol::proto as pb;
use nakama_spike::protocol::rest as api;
use nakama_spike::protocol::socket::{Envelope, MatchDataSend};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::{run_id, send};

const WAIT: Duration = Duration::from_secs(5);

fn login(cfg: &Config, device: &str) -> Result<api::Session, String> {
    let (r, _) = send(&api::authenticate_device(&cfg.base_url, &cfg.server_key, device, true))?;
    api::parse_session(&r).map_err(|e| e.to_string())
}

/// A protobuf-format socket: binary frames carrying `nakama.realtime.Envelope`.
struct PbSocket(WebSocketStream<TcpStream>);

impl PbSocket {
    async fn connect(base: &str, token: &str) -> Result<PbSocket, String> {
        let hp = base.strip_prefix("http://").ok_or("http:// only")?.trim_end_matches('/');
        let tcp = TcpStream::connect(hp).await.map_err(|e| e.to_string())?;
        tcp.set_nodelay(true).map_err(|e| e.to_string())?;
        let req = format!("ws://{hp}/ws?token={token}&format=protobuf").into_client_request().map_err(|e| e.to_string())?;
        let (ws, _) = tokio_tungstenite::client_async(req, tcp).await.map_err(|e| format!("websocket handshake: {e}"))?;
        Ok(PbSocket(ws))
    }
    async fn send(&mut self, e: &pb::Envelope) -> Result<(), String> {
        self.0.send(Message::binary(e.to_bytes())).await.map_err(|e| e.to_string())
    }
    async fn next(&mut self) -> Result<pb::Envelope, String> {
        loop {
            match tokio::time::timeout(WAIT, self.0.next()).await {
                Err(_) => return Err("timed out waiting for a message".into()),
                Ok(None) => return Err("socket closed".into()),
                Ok(Some(Err(e))) => return Err(e.to_string()),
                Ok(Some(Ok(Message::Binary(b)))) => return pb::Envelope::parse(&b),
                Ok(Some(Ok(Message::Close(c)))) => return Err(format!("closed by the server: {c:?}")),
                Ok(Some(Ok(_))) => {}
            }
        }
    }
    async fn request(&mut self, mut e: pb::Envelope) -> Result<pb::Envelope, String> {
        e.cid = "1".into();
        self.send(&e).await?;
        loop {
            let r = self.next().await?;
            if r.cid == "1" {
                if let Some(pb::envelope::Message::Error(er)) = &r.message {
                    return Err(format!("{}: {}", er.code, er.message));
                }
                return Ok(r);
            }
        }
    }
}

fn stats(label: &str, size: usize, wire: usize, mut r: Vec<f64>) {
    r.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p = |q: f64| r[((q * r.len() as f64).ceil() as usize).clamp(1, r.len()) - 1];
    println!("  {label:9} payload {size:5} B  wire {wire:5} B  RTT ms p50 {:.2} p95 {:.2} max {:.2} (n={})", p(0.5), p(0.95), r[r.len() - 1], r.len());
}

pub fn run(cfg: &Config, args: &[String]) -> Result<(), String> {
    let n = args.first().and_then(|v| v.parse().ok()).unwrap_or(300usize);
    tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?.block_on(e8(cfg, n))
}

async fn json_run(cfg: &Config, run: &str, size: usize, n: usize) -> Result<(usize, Vec<f64>), String> {
    let a = login(cfg, &format!("spike-j-a-{run}"))?;
    let b = login(cfg, &format!("spike-j-b-{run}"))?;
    let mut sa = Socket::connect(&cfg.base_url, &a.token).await?;
    let mut sb = Socket::connect(&cfg.base_url, &b.token).await?;
    let m = sa.request(Envelope::match_create("", None), WAIT).await?.matched.ok_or("no match")?;
    let jb = sb.request(Envelope::match_join("", &m.match_id), WAIT).await?.matched.ok_or("no join")?;
    let payload = vec![0x5Au8; size];
    let wire = Envelope::data(MatchDataSend::new(&m.match_id, 2, &payload, vec![jb.me.clone()])).to_json().len();
    let mut out = Vec::new();
    for _ in 0..n {
        let t = Instant::now();
        sa.send(&Envelope::data(MatchDataSend::new(&m.match_id, 2, &payload, vec![jb.me.clone()]))).await?;
        let d = sb.wait_for(WAIT, |e| e.match_data.as_ref().filter(|d| d.op_code == 2).map(|d| d.bytes())).await?.ok_or("no probe at B")?;
        sb.send(&Envelope::data(MatchDataSend::new(&m.match_id, 3, &d, vec![m.me.clone()]))).await?;
        sa.wait_for(WAIT, |e| e.match_data.as_ref().filter(|d| d.op_code == 3).map(|_| ())).await?.ok_or("no echo at A")?;
        out.push(t.elapsed().as_secs_f64() * 1000.0);
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    sa.close().await;
    sb.close().await;
    Ok((wire, out))
}

async fn proto_run(cfg: &Config, run: &str, size: usize, n: usize) -> Result<(usize, Vec<f64>), String> {
    let a = login(cfg, &format!("spike-p-a-{run}"))?;
    let b = login(cfg, &format!("spike-p-b-{run}"))?;
    let mut sa = PbSocket::connect(&cfg.base_url, &a.token).await?;
    let mut sb = PbSocket::connect(&cfg.base_url, &b.token).await?;
    let ma = match sa.request(pb::Envelope::match_create("")).await?.message {
        Some(pb::envelope::Message::Match(m)) => m,
        other => return Err(format!("create returned {other:?}")),
    };
    let mb = match sb.request(pb::Envelope::match_join("", &ma.match_id)).await?.message {
        Some(pb::envelope::Message::Match(m)) => m,
        other => return Err(format!("join returned {other:?}")),
    };
    let (a_pres, b_pres) = (ma.me.clone().ok_or("no self A")?, mb.me.clone().ok_or("no self B")?);
    let payload = vec![0x5Au8; size];
    let mk = |op: i64, data: &[u8], to: &pb::UserPresence| {
        pb::Envelope::data(pb::MatchDataSend { match_id: ma.match_id.clone(), op_code: op, data: data.to_vec(), presences: vec![to.clone()], reliable: true })
    };
    let wire = mk(2, &payload, &b_pres).to_bytes().len();
    let mut out = Vec::new();
    for _ in 0..n {
        let t = Instant::now();
        sa.send(&mk(2, &payload, &b_pres)).await?;
        let d = loop {
            if let Some(pb::envelope::Message::MatchData(d)) = sb.next().await?.message {
                if d.op_code == 2 {
                    break d.data;
                }
            }
        };
        sb.send(&mk(3, &d, &a_pres)).await?;
        loop {
            if let Some(pb::envelope::Message::MatchData(d)) = sa.next().await?.message {
                if d.op_code == 3 {
                    break;
                }
            }
        }
        out.push(t.elapsed().as_secs_f64() * 1000.0);
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    Ok((wire, out))
}

async fn e8(cfg: &Config, n: usize) -> Result<(), String> {
    println!("E8 JSON vs protobuf envelope: {n} sequential round trips per cell, A -> B -> A through the relay");
    let run = run_id();
    for size in [400usize, 2800, 3900] {
        match json_run(cfg, &run, size, n).await {
            Ok((wire, r)) => stats("json", size, wire, r),
            Err(e) => println!("  json      payload {size:5} B  FAILED: {e}"),
        }
        match proto_run(cfg, &run, size, n).await {
            Ok((wire, r)) => stats("protobuf", size, wire, r),
            Err(e) => println!("  protobuf  payload {size:5} B  FAILED: {e}"),
        }
    }
    Ok(())
}

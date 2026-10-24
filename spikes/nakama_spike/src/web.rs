//! Browser transport and the in-page checks (S3b, S4b): the same protocol layer as the native driver, driven by `ehttp` (browser
//! `fetch`) and `ewebsock` (the browser `WebSocket`). Functional checks, not a benchmark. The server key is passed in by the page
//! (from its URL query), never compiled in; session tokens are never put in the report.

use std::time::Duration;

use ewebsock::{Options, WsEvent, WsMessage, WsReceiver, WsSender};
use gloo_timers::future::sleep;
use wasm_bindgen::prelude::*;

use crate::protocol::rest as api;
use crate::protocol::socket::Envelope;

async fn fetch(req: ehttp::Request) -> Result<ehttp::Response, String> {
    let (tx, rx) = futures_channel::oneshot::channel();
    ehttp::fetch(req, move |r| {
        let _ = tx.send(r);
    });
    rx.await.map_err(|_| "fetch callback dropped".to_string())?
}

fn now() -> f64 {
    js_sys::Date::now()
}

struct Ws {
    tx: WsSender,
    rx: WsReceiver,
}

/// Wait until `pick` accepts an incoming envelope, or `wait_ms` passes. The browser delivers events to a queue; poll it every 2 ms.
async fn wait_for<T>(ws: &Ws, wait_ms: f64, mut pick: impl FnMut(&Envelope) -> Option<T>) -> Result<Option<T>, String> {
    let end = now() + wait_ms;
    while now() < end {
        while let Some(ev) = ws.rx.try_recv() {
            match ev {
                WsEvent::Message(WsMessage::Text(t)) => {
                    if let Some(v) = pick(&Envelope::parse(&t)?) {
                        return Ok(Some(v));
                    }
                }
                WsEvent::Error(e) => return Err(format!("socket error: {e}")),
                WsEvent::Closed => return Err("socket closed".into()),
                _ => {}
            }
        }
        sleep(Duration::from_millis(2)).await;
    }
    Ok(None)
}

async fn connect(base: &str, token: &str) -> Result<Ws, String> {
    let host = base.strip_prefix("http://").map(|h| ("ws", h)).or_else(|| base.strip_prefix("https://").map(|h| ("wss", h))).ok_or("bad base url")?;
    let url = format!("{}://{}/ws?token={token}&format=json", host.0, host.1.trim_end_matches('/'));
    let (tx, rx) = ewebsock::connect(url, Options::default())?;
    let ws = Ws { tx, rx };
    // The socket must be open before the first send; ewebsock reports `Opened`.
    let end = now() + 5000.0;
    while now() < end {
        match ws.rx.try_recv() {
            Some(WsEvent::Opened) => return Ok(ws),
            Some(WsEvent::Error(e)) => return Err(format!("socket error: {e}")),
            Some(WsEvent::Closed) => return Err("socket closed before opening".into()),
            _ => sleep(Duration::from_millis(2)).await,
        }
    }
    Err("socket did not open within 5 s".into())
}

async fn request(ws: &mut Ws, mut env: Envelope, cid: &str) -> Result<Envelope, String> {
    env.cid = Some(cid.to_string());
    ws.tx.send(WsMessage::Text(env.to_json()));
    let cid = cid.to_string();
    match wait_for(ws, 5000.0, |e| (e.cid.as_deref() == Some(cid.as_str())).then(|| e.clone())).await? {
        Some(reply) => match reply.error {
            Some(e) => Err(format!("{}: {}", e.code, e.message)),
            None => Ok(reply),
        },
        None => Err("no reply within 5 s".into()),
    }
}

/// Runs E1/E2 (REST) and a first E4 (socket, relayed match) from inside the page and returns a text report. A step that fails is
/// reported and stops the run; a CORS problem shows up as a failed first fetch.
#[wasm_bindgen]
pub async fn run_checks(base: String, server_key: String) -> String {
    let mut out = Vec::new();
    let r = checks(&base, &server_key, &mut out).await;
    if let Err(e) = r {
        out.push(format!("FAILED: {e}"));
    } else {
        out.push("ok".into());
    }
    out.join("\n")
}

async fn checks(base: &str, key: &str, out: &mut Vec<String>) -> Result<(), String> {
    let device = format!("spike-browser-{}", now() as u64);
    let mut step = |out: &mut Vec<String>, label: &str, t: f64, detail: String| out.push(format!("  {label:<44} {:7.0} ms  {detail}", now() - t));

    let t = now();
    let resp = fetch(api::authenticate_device(base, key, &device, true)).await.map_err(|e| format!("sign-in fetch failed (CORS or network): {e}"))?;
    let s = api::parse_session(&resp).map_err(|e| e.to_string())?;
    step(out, "E1 sign-in (cross-origin POST + preflight)", t, format!("created={}", s.created));

    let t = now();
    let s2 = api::parse_session(&fetch(api::refresh_session(base, key, &s.refresh_token)).await?).map_err(|e| e.to_string())?;
    step(out, "E1 refresh", t, "ok".into());

    let (_, uid) = api::claims(&s2.token).ok_or("cannot read the user id")?;
    let t = now();
    api::parse_write_ack(&fetch(api::storage_write(base, &s2.token, "spike_saves", "browser-slot", r#"{"hp":42}"#, None)).await?).map_err(|e| e.to_string())?;
    step(out, "E2 storage write (PUT)", t, "ok".into());
    let t = now();
    let got = api::parse_read(&fetch(api::storage_read(base, &s2.token, "spike_saves", "browser-slot", &uid)).await?).map_err(|e| e.to_string())?;
    step(out, "E2 storage read", t, format!("found={}", got.is_some()));

    let t = now();
    let mut ws = connect(base, &s2.token).await?;
    step(out, "E4 socket open (ewebsock)", t, String::new());
    let t = now();
    request(&mut ws, Envelope::ping(""), "1").await?;
    step(out, "E4 socket ping round trip", t, String::new());
    let t = now();
    let m = request(&mut ws, Envelope::match_create("", None), "2").await?.matched.ok_or("create returned no match")?;
    step(out, "E4 create a relayed match", t, format!("authoritative={}", m.authoritative));
    request(&mut ws, Envelope::match_leave("", &m.match_id), "3").await?;
    step(out, "E4 leave", now(), String::new());
    Ok(())
}

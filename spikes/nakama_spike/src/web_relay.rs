//! E5 relay client for the browser page (calibration C3/C4 and the browser grid): the same protocol as `e5.rs`, in wasm.
//!
//! Design points that differ from the native harness:
//! - Receive handling happens off the `WebSocket` callback, not at a main-loop poll: `ewebsock` calls our wake-up before it queues
//!   the event, so the wake-up stamps `performance.now()` and a microtask (which runs right after the callback returns) drains the
//!   queue, pairing each event with its stamp. A probe is echoed from there, with no timer in between.
//! - Sends are on a deadline schedule driven by `setTimeout` (the browser's timer granularity is the cost, so the lateness of every
//!   send against its deadline is reported next to the round trips). A probe is still timestamped from its SCHEDULED time.
//! - A page needs the tab visible and unthrottled (the driver launches Chromium with the background-throttling switches).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Duration;

use ewebsock::{Options, WsEvent, WsMessage, WsReceiver, WsSender};
use gloo_timers::future::sleep;
use wasm_bindgen::prelude::*;

use crate::protocol::rest as api;
use crate::protocol::socket::{Envelope, MatchDataSend, Presence};
use crate::relay::{payload, sched_of, OP_ECHO, OP_LOAD, OP_PROBE};
use crate::web::{fetch, request, Ws};

const WARMUP_MS: f64 = 5000.0;

fn perf_now() -> f64 {
    web_sys::window().expect("a window").performance().expect("performance").now()
}

struct Relay {
    tx: WsSender,
    rx: WsReceiver,
    match_id: String,
    me: Presence,
    peers: Vec<Presence>,
    started: bool,
    t0: f64,
    warm_end: f64,
    rtts: Vec<f64>,
    recv: u64,
    last_rx: Option<f64>,
    max_gap: f64,
    error: Option<String>,
}

thread_local! {
    static RELAY: RefCell<Option<Relay>> = const { RefCell::new(None) };
    static STAMPS: RefCell<VecDeque<f64>> = const { RefCell::new(VecDeque::new()) };
}

fn drain() {
    RELAY.with(|r| {
        let mut b = r.borrow_mut();
        let Some(rl) = b.as_mut() else { return };
        while let Some(ev) = rl.rx.try_recv() {
            let at = STAMPS.with(|s| s.borrow_mut().pop_front()).unwrap_or_else(perf_now);
            handle(rl, ev, at);
        }
    });
}

fn handle(rl: &mut Relay, ev: WsEvent, at: f64) {
    match ev {
        WsEvent::Message(WsMessage::Text(t)) => {
            let env = match Envelope::parse(&t) {
                Ok(e) => e,
                Err(e) => {
                    rl.error.get_or_insert(e);
                    return;
                }
            };
            if let Some(p) = env.match_presence_event {
                for j in p.joins {
                    if j.session_id != rl.me.session_id && !rl.peers.iter().any(|x| x.session_id == j.session_id) {
                        rl.peers.push(j);
                    }
                }
            }
            if let Some(d) = env.match_data {
                if rl.started && at >= rl.warm_end {
                    if let Some(last) = rl.last_rx {
                        rl.max_gap = rl.max_gap.max(at - last);
                    }
                    rl.last_rx = Some(at);
                    rl.recv += 1;
                }
                match d.op_code {
                    OP_PROBE => {
                        if let Some(from) = d.presence.clone() {
                            let m = MatchDataSend::new(&rl.match_id, OP_ECHO, &d.bytes(), vec![from]);
                            rl.tx.send(WsMessage::Text(Envelope::data(m).to_json()));
                        }
                    }
                    OP_ECHO if rl.started => {
                        if let Some(sched_us) = sched_of(&d.bytes()) {
                            let sched_at = rl.t0 + sched_us as f64 / 1000.0;
                            if sched_at >= rl.warm_end {
                                rl.rtts.push(at - sched_at);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        WsEvent::Error(e) => {
            rl.error.get_or_insert(format!("socket error: {e}"));
        }
        WsEvent::Closed => {
            rl.error.get_or_insert(format!("socket closed (at {:.1} s)", (at - rl.t0) / 1000.0));
        }
        _ => {}
    }
}

fn set_global(name: &str, value: &str) {
    let _ = js_sys::Reflect::set(&js_sys::global(), &JsValue::from_str(name), &JsValue::from_str(value));
}

fn send_text(text: String) {
    RELAY.with(|r| {
        if let Some(rl) = r.borrow_mut().as_mut() {
            rl.tx.send(WsMessage::Text(text));
        }
    });
}

/// Run one relay client. `match_id` empty = create the match (and publish its id as `window.__matchId`), otherwise join it.
/// Waits for `expected_peers` other members, then runs the schedules for `secs` and returns the statistics as JSON.
#[wasm_bindgen]
pub async fn run_relay(base: String, key: String, match_id: String, expected_peers: u32, load_hz: f64, probe_hz: f64, payload_bytes: u32, secs: u32) -> String {
    match relay(&base, &key, &match_id, expected_peers as usize, load_hz, probe_hz, payload_bytes as usize, secs as f64).await {
        Ok(json) => json,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
async fn relay(base: &str, key: &str, match_id: &str, expected_peers: usize, load_hz: f64, probe_hz: f64, size: usize, secs: f64) -> Result<String, String> {
    // ---- sign in, open the socket (with the wake-up that stamps arrivals)
    let device = format!("spike-web-{}-{}", js_sys::Date::now() as u64, (js_sys::Math::random() * 1e9) as u64);
    let s = api::parse_session(&fetch(api::authenticate_device(base, key, &device, true)).await?).map_err(|e| e.to_string())?;
    let scheme = if base.starts_with("https://") { "wss" } else { "ws" };
    let host = base.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/');
    let url = format!("{scheme}://{host}/ws?token={}&format=json", s.token);
    let (tx, rx) = ewebsock::connect_with_wakeup(url, Options::default(), || {
        STAMPS.with(|st| st.borrow_mut().push_back(perf_now()));
        wasm_bindgen_futures::spawn_local(async { drain() });
    })?;
    let mut ws = Ws { tx, rx };
    let end = perf_now() + 5000.0;
    loop {
        match ws.rx.try_recv() {
            Some(WsEvent::Opened) => break,
            Some(WsEvent::Error(e)) => return Err(format!("socket error: {e}")),
            Some(WsEvent::Closed) => return Err("socket closed before opening".into()),
            _ if perf_now() > end => return Err("socket did not open within 5 s".into()),
            _ => sleep(Duration::from_millis(2)).await,
        }
    }

    // ---- create or join the match
    let reply = if match_id.is_empty() {
        request(&mut ws, Envelope::match_create("", None), "1").await?
    } else {
        request(&mut ws, Envelope::match_join("", match_id), "1").await?
    };
    let m = reply.matched.ok_or("no match in the reply")?;
    set_global("__matchId", &m.match_id);

    // ---- hand the socket to the callback-driven state
    let me = m.me.clone();
    RELAY.with(|r| {
        *r.borrow_mut() = Some(Relay {
            tx: ws.tx,
            rx: ws.rx,
            match_id: m.match_id.clone(),
            me: me.clone(),
            peers: m.presences.into_iter().filter(|p| p.session_id != me.session_id).collect(),
            started: false,
            t0: 0.0,
            warm_end: f64::MAX,
            rtts: Vec::new(),
            recv: 0,
            last_rx: None,
            max_gap: 0.0,
            error: None,
        });
    });
    STAMPS.with(|s| s.borrow_mut().clear());
    drain();

    // ---- wait for everybody, then start the schedules
    let wait_end = perf_now() + 120_000.0;
    let (target, t0) = loop {
        let ready = RELAY.with(|r| r.borrow().as_ref().map(|rl| rl.peers.len() >= expected_peers).unwrap_or(false));
        if ready {
            let t0 = perf_now() + 300.0;
            let target = RELAY.with(|r| {
                let mut b = r.borrow_mut();
                let rl = b.as_mut().expect("relay state");
                rl.t0 = t0;
                rl.warm_end = t0 + WARMUP_MS;
                rl.started = true;
                let mut all: Vec<Presence> = rl.peers.clone();
                all.push(rl.me.clone());
                all.sort_by(|a, b| a.session_id.cmp(&b.session_id));
                let pos = all.iter().position(|p| p.session_id == rl.me.session_id).unwrap_or(0);
                all[(pos + 1) % all.len()].clone()
            });
            break (target, t0);
        }
        if perf_now() > wait_end {
            return Err("the other members did not join within 120 s".into());
        }
        sleep(Duration::from_millis(20)).await;
    };

    // ---- run: deadline-scheduled load and probes
    let (mut load_n, mut probe_n) = (0u64, 0u64);
    let (mut late_max, mut late_sum, mut late_count) = (0.0f64, 0.0f64, 0u64);
    let match_id = RELAY.with(|r| r.borrow().as_ref().map(|rl| rl.match_id.clone()).unwrap_or_default());
    let end_ms = secs * 1000.0;
    loop {
        let next_load = if load_hz > 0.0 { load_n as f64 * 1000.0 / load_hz } else { f64::INFINITY };
        let next_probe = if probe_hz > 0.0 { probe_n as f64 * 1000.0 / probe_hz } else { f64::INFINITY };
        let due = next_load.min(next_probe);
        if due >= end_ms {
            break;
        }
        let remaining = due - (perf_now() - t0);
        if remaining >= 1.0 {
            sleep(Duration::from_millis(remaining.floor() as u64)).await;
            continue;
        }
        let late = (perf_now() - t0 - due).max(0.0);
        late_max = late_max.max(late);
        late_sum += late;
        late_count += 1;
        if next_load <= next_probe {
            let m = MatchDataSend::new(&match_id, OP_LOAD, &payload(load_n, (due * 1000.0) as u64, size), vec![]);
            send_text(Envelope::data(m).to_json());
            load_n += 1;
        } else {
            let m = MatchDataSend::new(&match_id, OP_PROBE, &payload(probe_n, (due * 1000.0) as u64, size), vec![target.clone()]);
            send_text(Envelope::data(m).to_json());
            probe_n += 1;
        }
    }
    sleep(Duration::from_millis(1500)).await;

    RELAY.with(|r| {
        let b = r.borrow();
        let rl = b.as_ref().expect("relay state");
        Ok(serde_json::json!({
            "rtts": rl.rtts,
            "recv": rl.recv,
            "max_gap_ms": rl.max_gap,
            "late_max_ms": late_max,
            "late_mean_ms": if late_count > 0 { late_sum / late_count as f64 } else { 0.0 },
            "sent_probes": probe_n,
            "sent_load": load_n,
            "error": rl.error,
        })
        .to_string())
    })
}

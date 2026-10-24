//! E5 (native part): relay round-trip latency, stalls and disconnects, following the plan's protocol.
//!
//!   nakama_spike e5 [clients=2] [load_hz=0] [probe_hz=30] [payload_bytes=64] [secs=90]
//!
//! Defaults are calibration C2 (native to native, probes only). Design points, all from the plan:
//! - Load traffic (op 1, broadcast to everyone) and probe traffic (op 2, to ONE target; the target echoes it as op 3 to the sender
//!   only) are separate schedules, so echoes never multiply the load.
//! - Sends are scheduled by deadline (`start + n * period`) and a probe is timestamped from its SCHEDULED send time, so a stall
//!   cannot hide itself by delaying the next send (no coordinated omission).
//! - A receive is timestamped inside the reader task the moment it is read from the socket; each client has a writer task and a
//!   reader task (split socket, `TCP_NODELAY`). Intervals use `std::time::Instant`.
//! - The first 5 s are discarded as warm-up. The run is longer than the 60 s session token, which also shows whether an open
//!   socket survives its token's expiry.
//! - p99 is only reported above 2000 samples. Tokens and URLs are never printed.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tokio::time::sleep_until;

use nakama_spike::config::Config;
use nakama_spike::native::Socket;
use nakama_spike::protocol::rest as api;
use nakama_spike::protocol::socket::{Envelope, MatchDataSend, Presence};

use crate::{run_id, send};

const OP_LOAD: i64 = 1;
const OP_PROBE: i64 = 2;
const OP_ECHO: i64 = 3;
const WARMUP: Duration = Duration::from_secs(5);

#[derive(Clone, Copy)]
struct Params {
    clients: usize,
    load_hz: f64,
    probe_hz: f64,
    payload: usize,
    secs: u64,
}

#[derive(Default)]
struct Stats {
    rtts_ms: Vec<f64>,
    recv: u64,
    max_gap_ms: f64,
    last_rx: Option<Instant>,
    error: Option<String>,
}

#[derive(Default)]
struct Counters {
    enqueued: AtomicU64,
    written: AtomicU64,
}

fn arg<T: std::str::FromStr>(args: &[String], i: usize, default: T) -> T {
    args.get(i).and_then(|a| a.parse().ok()).unwrap_or(default)
}

pub fn run(cfg: &Config, args: &[String]) -> Result<(), String> {
    let p = Params {
        clients: arg(args, 0, 2usize).max(2),
        load_hz: arg(args, 1, 0.0),
        probe_hz: arg(args, 2, 30.0),
        payload: arg(args, 3, 64usize).max(16),
        secs: arg(args, 4, 90u64),
    };
    tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().map_err(|e| e.to_string())?.block_on(e5(cfg, p))
}

/// 16-byte header (sequence, scheduled send time in microseconds since the run start) padded to `size` bytes.
fn payload(seq: u64, sched_us: u64, size: usize) -> Vec<u8> {
    let mut v = vec![0u8; size.max(16)];
    v[..8].copy_from_slice(&seq.to_le_bytes());
    v[8..16].copy_from_slice(&sched_us.to_le_bytes());
    v
}

fn sched_of(bytes: &[u8]) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(8..16)?.try_into().ok()?))
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = ((p * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    sorted[rank - 1]
}

async fn e5(cfg: &Config, p: Params) -> Result<(), String> {
    println!(
        "E5 relay: {} clients, load {} Hz, probes {} Hz, payload {} B, {} s (warm-up {} s discarded)",
        p.clients,
        p.load_hz,
        p.probe_hz,
        p.payload,
        p.secs,
        WARMUP.as_secs()
    );

    // ---- setup: sign in, connect, one match, everybody joined
    let run = run_id();
    let mut sockets = Vec::new();
    let mut presences: Vec<Presence> = Vec::new();
    let mut match_id = String::new();
    for i in 0..p.clients {
        let (r, _) = send(&api::authenticate_device(&cfg.base_url, &cfg.server_key, &format!("spike-e5-{run}-{i}"), true))?;
        let s = api::parse_session(&r).map_err(|e| e.to_string())?;
        let mut sock = Socket::connect(&cfg.base_url, &s.token).await?;
        let reply = if i == 0 {
            sock.request(Envelope::match_create("", None), Duration::from_secs(5)).await?
        } else {
            sock.request(Envelope::match_join("", &match_id), Duration::from_secs(5)).await?
        };
        let m = reply.matched.ok_or("no match in the reply")?;
        if i == 0 {
            match_id = m.match_id.clone();
        }
        presences.push(m.me.clone());
        sockets.push(sock);
    }
    let presences = Arc::new(presences);

    // ---- run
    let start = Instant::now() + Duration::from_millis(500);
    let tstart = tokio::time::Instant::from_std(start);
    let warm_end = start + WARMUP;
    let stats: Vec<Arc<Mutex<Stats>>> = (0..p.clients).map(|_| Arc::default()).collect();
    let counters = Arc::new(Counters::default());
    let mut tasks = Vec::new();
    let sent_probes = Arc::new(AtomicU64::new(0));
    let sent_load = Arc::new(AtomicU64::new(0));

    for (i, sock) in sockets.into_iter().enumerate() {
        let (mut writer, mut reader) = sock.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();

        // writer task
        let c = counters.clone();
        let st = stats[i].clone();
        tasks.push(tokio::spawn(async move {
            while let Some(text) = rx.recv().await {
                if let Err(e) = writer.send_text(text).await {
                    st.lock().unwrap().error.get_or_insert(format!("write: {e}"));
                    break;
                }
                c.written.fetch_add(1, Relaxed);
            }
        }));

        // reader task: timestamps on receipt, echoes probes
        let st = stats[i].clone();
        let echo_tx = tx.clone();
        let c = counters.clone();
        let mid = match_id.clone();
        tasks.push(tokio::spawn(async move {
            loop {
                match reader.next().await {
                    Err(e) => {
                        st.lock().unwrap().error.get_or_insert(format!("read: {e} (after {:.1} s)", start.elapsed().as_secs_f64()));
                        break;
                    }
                    Ok((at, env)) => {
                        let Some(d) = env.match_data else { continue };
                        let mut s = st.lock().unwrap();
                        if at >= warm_end {
                            if let Some(last) = s.last_rx {
                                s.max_gap_ms = s.max_gap_ms.max(at.duration_since(last).as_secs_f64() * 1000.0);
                            }
                            s.last_rx = Some(at);
                            s.recv += 1;
                        }
                        match d.op_code {
                            OP_PROBE => {
                                drop(s);
                                if let Some(from) = d.presence.clone() {
                                    let m = MatchDataSend::new(&mid, OP_ECHO, &d.bytes(), vec![from]);
                                    c.enqueued.fetch_add(1, Relaxed);
                                    let _ = echo_tx.send(Envelope::data(m).to_json());
                                }
                            }
                            OP_ECHO => {
                                if let Some(sched) = sched_of(&d.bytes()) {
                                    let sched_at = start + Duration::from_micros(sched);
                                    if sched_at >= warm_end {
                                        let rtt = at.saturating_duration_since(sched_at).as_secs_f64() * 1000.0;
                                        s.rtts_ms.push(rtt);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }));

        // load sender: broadcast to everyone, on a deadline schedule
        if p.load_hz > 0.0 {
            let (tx, c, sent, mid) = (tx.clone(), counters.clone(), sent_load.clone(), match_id.clone());
            tasks.push(tokio::spawn(async move {
                let period = Duration::from_secs_f64(1.0 / p.load_hz);
                for n in 0u64.. {
                    let due = period * n as u32;
                    if due.as_secs() >= p.secs {
                        break;
                    }
                    sleep_until(tstart + due).await;
                    let m = MatchDataSend::new(&mid, OP_LOAD, &payload(n, due.as_micros() as u64, p.payload), vec![]);
                    c.enqueued.fetch_add(1, Relaxed);
                    sent.fetch_add(1, Relaxed);
                    let _ = tx.send(Envelope::data(m).to_json());
                }
            }));
        }

        // probe sender: to the next client in the ring only, timestamped from the SCHEDULED time
        if p.probe_hz > 0.0 {
            let (tx, c, sent, mid, pres) = (tx.clone(), counters.clone(), sent_probes.clone(), match_id.clone(), presences.clone());
            tasks.push(tokio::spawn(async move {
                let target = pres[(i + 1) % pres.len()].clone();
                let period = Duration::from_secs_f64(1.0 / p.probe_hz);
                for n in 0u64.. {
                    let due = period * n as u32;
                    if due.as_secs() >= p.secs {
                        break;
                    }
                    sleep_until(tstart + due).await;
                    let m = MatchDataSend::new(&mid, OP_PROBE, &payload(n, due.as_micros() as u64, p.payload), vec![target.clone()]);
                    c.enqueued.fetch_add(1, Relaxed);
                    sent.fetch_add(1, Relaxed);
                    let _ = tx.send(Envelope::data(m).to_json());
                }
            }));
        }
        // `tx` (the original) is dropped here; the writer task ends when every sender has finished.
    }

    tokio::time::sleep_until(tstart + Duration::from_secs(p.secs) + Duration::from_millis(1500)).await;

    // ---- report
    let mut all: Vec<f64> = Vec::new();
    let mut errors = 0;
    for (i, st) in stats.iter().enumerate() {
        let s = st.lock().unwrap();
        println!(
            "  client {i}: {} RTT samples, {} msgs received after warm-up, longest gap between received messages {:.0} ms{}",
            s.rtts_ms.len(),
            s.recv,
            s.max_gap_ms,
            s.error.as_ref().map(|e| format!(", ERROR {e}")).unwrap_or_default()
        );
        if s.error.is_some() {
            errors += 1;
        }
        all.extend(&s.rtts_ms);
    }
    for t in &tasks {
        t.abort();
    }
    let expected_probe = p.probe_hz * p.secs as f64 * p.clients as f64;
    let expected_load = p.load_hz * p.secs as f64 * p.clients as f64;
    println!(
        "  achieved send rate: probes {:.1}% of {:.0}, load {:.1}% of {:.0}; enqueued {} written {} (a gap means the writer lagged)",
        if expected_probe > 0.0 { sent_probes.load(Relaxed) as f64 / expected_probe * 100.0 } else { 100.0 },
        expected_probe,
        if expected_load > 0.0 { sent_load.load(Relaxed) as f64 / expected_load * 100.0 } else { 100.0 },
        expected_load,
        counters.enqueued.load(Relaxed),
        counters.written.load(Relaxed)
    );
    if all.is_empty() {
        return Err("no round-trip samples were collected".into());
    }
    all.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = all.len();
    let mean = all.iter().sum::<f64>() / n as f64;
    let p99 = if n >= 2000 { format!("{:.2}", percentile(&all, 0.99)) } else { format!("n/a (only {n} samples, p99 needs 2000)") };
    println!(
        "  RTT ms: n={n} min {:.2} p50 {:.2} mean {:.2} p95 {:.2} p99 {p99} max {:.2}",
        all[0],
        percentile(&all, 0.5),
        mean,
        percentile(&all, 0.95),
        all[n - 1]
    );
    println!("  disconnects/errors: {errors} (run length {} s, longer than the 60 s session token)", p.secs);
    if errors > 0 {
        return Err("a client disconnected or errored during the run".into());
    }
    Ok(())
}

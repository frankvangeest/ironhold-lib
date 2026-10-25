//! E6 (matchmaker) and E7 (drop and reconnect, token expiry, soak). Functional experiments: printed times are one-shot.
//!   nakama_spike e6
//!   nakama_spike e7            forced drop + reconnect, expired token on connect
//!   nakama_spike e7 soak [secs=600]   two clients in a match, a probe per second, ping every 15 s, a session refresh every 45 s

use std::time::{Duration, Instant};

use nakama_spike::config::Config;
use nakama_spike::native::Socket;
use nakama_spike::protocol::rest as api;
use nakama_spike::protocol::socket::{Envelope, MatchDataSend, Presence};

use crate::{run_id, send, show};

const WAIT: Duration = Duration::from_secs(5);

fn login(cfg: &Config, device: &str) -> Result<api::Session, String> {
    let (r, _) = send(&api::authenticate_device(&cfg.base_url, &cfg.server_key, device, true))?;
    api::parse_session(&r).map_err(|e| e.to_string())
}

fn refresh(cfg: &Config, s: &api::Session) -> Result<api::Session, String> {
    let (r, _) = send(&api::refresh_session(&cfg.base_url, &cfg.server_key, &s.refresh_token))?;
    api::parse_session(&r).map_err(|e| e.to_string())
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())
}

pub fn run_e6(cfg: &Config) -> Result<(), String> {
    rt()?.block_on(e6(cfg))
}

async fn e6(cfg: &Config) -> Result<(), String> {
    println!("E6 matchmaker (the server's default interval is 15 s; time to match is recorded, not judged)");
    let run = run_id();
    let a = login(cfg, &format!("spike-mm-a-{run}"))?;
    let b = login(cfg, &format!("spike-mm-b-{run}"))?;
    let mut sa = Socket::connect(&cfg.base_url, &a.token).await?;
    let mut sb = Socket::connect(&cfg.base_url, &b.token).await?;

    let t = Instant::now();
    let ta = sa.request(Envelope::matchmaker_add("", 2, 2, "*"), WAIT).await?.matchmaker_ticket.ok_or("no ticket for A")?;
    let tb = sb.request(Envelope::matchmaker_add("", 2, 2, "*"), WAIT).await?.matchmaker_ticket.ok_or("no ticket for B")?;
    show("both clients add a ticket (min 2, max 2, query *)", ms(t), format!("tickets differ: {}", ta.ticket != tb.ticket));

    let t = Instant::now();
    let ma = sa.wait_for(Duration::from_secs(60), |e| e.matchmaker_matched.clone()).await?.ok_or("A was not matched within 60 s")?;
    let a_after = ms(t);
    let mb = sb.wait_for(Duration::from_secs(60), |e| e.matchmaker_matched.clone()).await?.ok_or("B was not matched within 60 s")?;
    show("A is told it was matched (time from adding the tickets)", a_after, format!("{} user(s) in the result, token present: {}, match_id present: {}", ma.users.len(), !ma.token.is_empty(), !ma.match_id.is_empty()));
    show("B is told it was matched", ms(t), format!("same ticket-to-result mapping: {}", mb.ticket == tb.ticket));

    let t = Instant::now();
    let joined_a = sa.request(Envelope::match_join_token("", &ma.token), WAIT).await?.matched.ok_or("A's token join returned no match")?;
    let joined_b = sb.request(Envelope::match_join_token("", &mb.token), WAIT).await?.matched.ok_or("B's token join returned no match")?;
    show("both join the match with their matchmaker token", ms(t), format!("same match: {}, authoritative={}", joined_a.match_id == joined_b.match_id, joined_a.authoritative));
    sa.send(&Envelope::data(MatchDataSend::new(&joined_a.match_id, 1, b"matched", vec![]))).await?;
    let got = sb.wait_for(WAIT, |e| e.match_data.as_ref().map(|d| d.bytes())).await?;
    show("data flows in the matched match", 0.0, format!("{:?}", got.map(|b| String::from_utf8_lossy(&b).to_string())));
    sa.close().await;
    sb.close().await;
    Ok(())
}

pub fn run_e7(cfg: &Config, args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) == Some("soak") {
        let secs = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(600u64);
        rt()?.block_on(soak(cfg, secs))
    } else {
        rt()?.block_on(e7(cfg))
    }
}

async fn join_pair(cfg: &Config, a: &api::Session, b: &api::Session) -> Result<(Socket, Socket, String, Presence, Presence), String> {
    let mut sa = Socket::connect(&cfg.base_url, &a.token).await?;
    let mut sb = Socket::connect(&cfg.base_url, &b.token).await?;
    let m = sa.request(Envelope::match_create("", None), WAIT).await?.matched.ok_or("create returned no match")?;
    let jb = sb.request(Envelope::match_join("", &m.match_id), WAIT).await?.matched.ok_or("join returned no match")?;
    Ok((sa, sb, m.match_id, m.me, jb.me))
}

async fn e7(cfg: &Config) -> Result<(), String> {
    println!("E7 drop and reconnect");
    let run = run_id();
    let a = login(cfg, &format!("spike-rc-a-{run}"))?;
    let b = login(cfg, &format!("spike-rc-b-{run}"))?;
    let (sa, mut sb, match_id, _a_pres, _b_pres) = join_pair(cfg, &a, &b).await?;
    let a_uid = api::claims(&a.token).ok_or("no claims")?.1;

    // A's connection dies without a close handshake (the socket is just dropped), as with a pulled cable or a closed laptop lid.
    let t = Instant::now();
    drop(sa);
    let left = sb.wait_for(Duration::from_secs(30), |e| e.match_presence_event.as_ref().filter(|p| p.leaves.iter().any(|l| l.user_id == a_uid)).map(|_| ())).await?;
    show("abrupt drop of A: B sees A's leave event after", ms(t), format!("{} (a quiet socket is detected by the server, not instantly)", left.is_some()));

    // The client recovers: a fresh token (the old one may have expired), a new socket, the same match.
    let t = Instant::now();
    let a2 = refresh(cfg, &a)?;
    let t_refresh = ms(t);
    let mut sa2 = Socket::connect(&cfg.base_url, &a2.token).await?;
    let t_conn = ms(t);
    let rejoined = sa2.request(Envelope::match_join("", &match_id), WAIT).await?.matched.ok_or("rejoin returned no match")?;
    show(
        "A recovers: refresh, new socket, rejoin the same match",
        ms(t),
        format!("refresh {t_refresh:.0} ms, socket at {t_conn:.0} ms, rejoined same match: {}, size {} (criterion: within 5 s)", rejoined.match_id == match_id, rejoined.size),
    );
    let back = sb.wait_for(WAIT, |e| e.match_presence_event.as_ref().filter(|p| p.joins.iter().any(|j| j.user_id == a_uid)).map(|_| ())).await?;
    show("B sees A rejoin", 0.0, back.is_some());
    sa2.send(&Envelope::data(MatchDataSend::new(&match_id, 1, b"back", vec![]))).await?;
    let got = sb.wait_for(WAIT, |e| e.match_data.as_ref().map(|d| d.bytes())).await?;
    show("data flows again after the reconnect", 0.0, got.is_some());

    // Expired token: a socket needs a valid session token at connect time. The token lives 60 s.
    println!("waiting 65 s for a session token to expire ...");
    let old = login(cfg, &format!("spike-rc-old-{run}"))?;
    tokio::time::sleep(Duration::from_secs(65)).await;
    match Socket::connect(&cfg.base_url, &old.token).await {
        Ok(s) => {
            show("connect with an EXPIRED token", 0.0, "accepted (unexpected)");
            s.close().await;
        }
        Err(e) => show("connect with an EXPIRED token", 0.0, format!("refused: {e}")),
    }
    let fresh = refresh(cfg, &old)?;
    let s = Socket::connect(&cfg.base_url, &fresh.token).await?;
    show("connect after refreshing the session", 0.0, "accepted");
    s.close().await;
    sa2.close().await;
    sb.close().await;
    Ok(())
}

async fn soak(cfg: &Config, secs: u64) -> Result<(), String> {
    println!("E7 soak: {secs} s, a probe per second, ping every 15 s, a session refresh every 45 s");
    let run = run_id();
    let mut a = login(cfg, &format!("spike-soak-a-{run}"))?;
    let b = login(cfg, &format!("spike-soak-b-{run}"))?;
    let (mut sa, mut sb, match_id, _a_pres, b_pres) = join_pair(cfg, &a, &b).await?;
    let start = Instant::now();
    let (mut next_probe, mut next_ping, mut next_refresh) = (Duration::ZERO, Duration::from_secs(15), Duration::from_secs(45));
    let (mut sent, mut got, mut pings, mut refreshes, mut max_rtt) = (0u32, 0u32, 0u32, 0u32, 0.0f64);
    let mut sent_at = std::collections::HashMap::new();
    while start.elapsed() < Duration::from_secs(secs) {
        let now = start.elapsed();
        if now >= next_probe {
            let m = MatchDataSend::new(&match_id, 2, &sent.to_le_bytes(), vec![b_pres.clone()]);
            sent_at.insert(sent, Instant::now());
            sa.send(&Envelope::data(m)).await.map_err(|e| format!("A write failed at {:.0} s: {e}", now.as_secs_f64()))?;
            sent += 1;
            next_probe += Duration::from_secs(1);
        }
        if now >= next_ping {
            sa.request(Envelope::ping(""), WAIT).await.map_err(|e| format!("A ping failed at {:.0} s: {e}", now.as_secs_f64()))?;
            pings += 1;
            next_ping += Duration::from_secs(15);
        }
        if now >= next_refresh {
            a = refresh(cfg, &a).map_err(|e| format!("refresh failed at {:.0} s: {e}", now.as_secs_f64()))?;
            refreshes += 1;
            next_refresh += Duration::from_secs(45);
        }
        match sb.recv(Duration::from_millis(100)).await.map_err(|e| format!("B read failed at {:.0} s: {e}", now.as_secs_f64()))? {
            Some(e) => {
                if let Some(d) = e.match_data.filter(|d| d.op_code == 2) {
                    let b = d.bytes();
                    if b.len() >= 4 {
                        let n = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                        if let Some(t) = sent_at.remove(&n) {
                            got += 1;
                            max_rtt = max_rtt.max(ms(t));
                        }
                    }
                }
            }
            None => {}
        }
    }
    show("soak finished", 0.0, format!("{} s, probes sent {sent}, delivered {got}, pings {pings}, session refreshes {refreshes}, slowest delivery {max_rtt:.0} ms, no disconnect", start.elapsed().as_secs()));
    sa.close().await;
    sb.close().await;
    if got + 2 < sent {
        return Err(format!("{} probes were lost", sent - got));
    }
    Ok(())
}

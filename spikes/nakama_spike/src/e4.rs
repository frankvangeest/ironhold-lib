//! E4a/E4b/E4c: the realtime socket (JSON envelope), a relayed match between two users, what `ListMatches` returns for it, and a
//! lobby list built from public-read storage objects. Functional experiments: the printed times are one-shot, not a benchmark.

use std::time::{Duration, Instant};

use nakama_spike::config::Config;
use nakama_spike::native::Socket;
use nakama_spike::protocol::rest as api;
use nakama_spike::protocol::socket::{Envelope, MatchDataSend};

use crate::{run_id, send, show};

const WAIT: Duration = Duration::from_secs(5);

fn login(cfg: &Config, device: &str) -> Result<api::Session, String> {
    let (r, _) = send(&api::authenticate_device(&cfg.base_url, &cfg.server_key, device, true))?;
    api::parse_session(&r).map_err(|e| e.to_string())
}

/// A short, non-secret label of an id for the log.
fn short(id: &str) -> String {
    format!("{}...({} chars)", id.chars().take(8).collect::<String>(), id.len())
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

pub fn run(cfg: &Config) -> Result<(), String> {
    tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?.block_on(e4(cfg))
}

async fn e4(cfg: &Config) -> Result<(), String> {
    println!("E4 realtime socket (JSON envelope)");
    let run = run_id();
    let a = login(cfg, &format!("spike-a-{run}"))?;
    let b = login(cfg, &format!("spike-b-{run}"))?;
    let (_, a_uid) = api::claims(&a.token).ok_or("cannot read user A's id")?;

    let t = Instant::now();
    let mut sa = Socket::connect(&cfg.base_url, &a.token).await?;
    show("A connects the socket", ms(t), "");
    let mut sb = Socket::connect(&cfg.base_url, &b.token).await?;

    let t = Instant::now();
    let pong = sa.request(Envelope::ping(""), WAIT).await?;
    show("socket ping (calibration C1)", ms(t), format!("pong: {}", pong.pong.is_some()));

    // ---- E4a: unnamed relayed match
    let t = Instant::now();
    let m = sa.request(Envelope::match_create("", None), WAIT).await?.matched.ok_or("create returned no match")?;
    show("A creates a relayed match", ms(t), format!("id {} authoritative={} size={} label={:?}", short(&m.match_id), m.authoritative, m.size, m.label));

    let t = Instant::now();
    let joined = sb.request(Envelope::match_join("", &m.match_id), WAIT).await?.matched.ok_or("join returned no match")?;
    show("B joins by id", ms(t), format!("size={} presences seen by B={}", joined.size, joined.presences.len()));
    let b_pres = joined.me.clone();
    let uid = b_pres.user_id.clone();
    let seen = sa.wait_for(WAIT, |e| e.match_presence_event.as_ref().filter(|p| p.joins.iter().any(|j| j.user_id == uid)).map(|_| ())).await?;
    show("A sees B's join event", 0.0, seen.is_some());

    // broadcast: A -> everyone else
    let t = Instant::now();
    sa.send(&Envelope::data(MatchDataSend::new(&m.match_id, 1, b"hello", vec![]))).await?;
    let got = sb.wait_for(WAIT, |e| e.match_data.as_ref().map(|d| (d.op_code, d.bytes()))).await?;
    show(
        "A broadcasts op 1 'hello'; B receives",
        ms(t),
        format!("{:?}", got.as_ref().map(|(op, b)| (*op, String::from_utf8_lossy(b).to_string()))),
    );
    if got != Some((1, b"hello".to_vec())) {
        return Err("the broadcast did not arrive intact (op_code string / base64 round trip)".into());
    }
    let echo = sa.wait_for(Duration::from_millis(300), |e| e.match_data.as_ref().map(|_| ())).await?;
    show("does A get its own broadcast back?", 0.0, echo.is_some());

    // targeted: A -> B only
    let t = Instant::now();
    sa.send(&Envelope::data(MatchDataSend::new(&m.match_id, 2, b"only-b", vec![b_pres.clone()]))).await?;
    let got = sb.wait_for(WAIT, |e| e.match_data.as_ref().filter(|d| d.op_code == 2).map(|d| d.bytes())).await?;
    show("A sends op 2 to B only; B receives", ms(t), got.is_some());

    // ---- E4b: what does ListMatches say while this relayed match exists?
    for authoritative in [false, true] {
        let (r, took) = send(&api::match_list(&cfg.base_url, &b.token, authoritative, 10))?;
        let (status, body) = api::raw(&r);
        let n = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("matches").and_then(|m| m.as_array()).map(|m| m.len()))
            .unwrap_or(0);
        let head: String = body.chars().take(160).collect();
        show(&format!("E4b ListMatches authoritative={authoritative}"), took, format!("HTTP {status}, {n} match(es), body: {head}"));
    }

    // ---- E4c: a lobby list without server code: public-read storage objects
    let lobby = format!(r#"{{"host":"A","match_id":"{}","players":1,"max":4}}"#, m.match_id);
    let key = format!("lobby-{run}");
    let (r, took) = send(&api::storage_write_with(&cfg.base_url, &a.token, "lobbies", &key, &lobby, None, 2))?;
    api::parse_write_ack(&r).map_err(|e| e.to_string())?;
    show("E4c A publishes a public lobby object", took, "");
    let (r, took) = send(&api::storage_list_public(&cfg.base_url, &b.token, "lobbies", 20))?;
    let list = api::parse_storage_list(&r).map_err(|e| e.to_string())?;
    let visible = list.iter().any(|(owner, k, _)| owner == &a_uid && k == &key);
    show("E4c B lists the public lobbies", took, format!("{} object(s), A's lobby visible to B: {visible}", list.len()));
    if !visible {
        return Err("B cannot see A's public-read lobby object".into());
    }

    // ---- leave, then join a match whose last member left
    let t = Instant::now();
    sb.request(Envelope::match_leave("", &m.match_id), WAIT).await?;
    let uid = b_pres.user_id.clone();
    let left = sa.wait_for(WAIT, |e| e.match_presence_event.as_ref().filter(|p| p.leaves.iter().any(|l| l.user_id == uid)).map(|_| ())).await?;
    show("B leaves; A sees the leave event", ms(t), left.is_some());
    sa.request(Envelope::match_leave("", &m.match_id), WAIT).await?;
    let again = sb.request(Envelope::match_join("", &m.match_id), WAIT).await;
    let note = match &again {
        Ok(_) => "joined (the match still exists)".to_string(),
        Err(e) => format!("refused: {e}"),
    };
    show("B joins the match after its last member left", 0.0, note);

    // ---- named match: stable id
    let name = format!("spike-named-{run}");
    let n1 = sa.request(Envelope::match_create("", Some(&name)), WAIT).await?.matched.ok_or("named create 1")?;
    let n2 = sb.request(Envelope::match_create("", Some(&name)), WAIT).await?.matched.ok_or("named create 2")?;
    show("named create by A, then by B (same name)", 0.0, format!("same match id: {}, size seen by B: {}", n1.match_id == n2.match_id, n2.size));
    sa.close().await;
    sb.close().await;
    Ok(())
}

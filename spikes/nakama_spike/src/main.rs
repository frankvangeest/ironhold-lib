//! Native driver for the Nakama spike (planning/features/nakama_backend_spike.md). Experiments are subcommands:
//!
//!   nakama_spike probe   reachability + "the default server key is rejected" (needs no key)
//!   nakama_spike e1      REST: device-id sign-in (create + re-login) and refresh
//!   nakama_spike e2      REST: storage write / read / version conflict
//!   nakama_spike all     probe, e1, e2
//!
//! Secrets: the endpoint is `NAKAMA_URL`; the server key is `NAKAMA_SERVER_KEY` or a file outside the repo (see config.rs).
//! Nothing secret is ever printed: session tokens are reported by length and lifetime only.

mod api;
mod config;

use std::time::Instant;

use api::ApiError;

fn send(req: &ehttp::Request) -> Result<(ehttp::Response, f64), String> {
    let t = Instant::now();
    let resp = ehttp::fetch_blocking(req)?;
    Ok((resp, t.elapsed().as_secs_f64() * 1000.0))
}

fn show(label: &str, ms: f64, detail: impl std::fmt::Display) {
    println!("  {label:<46} {ms:7.1} ms  {detail}");
}

/// A unique id per run so a re-run never collides with an earlier test account's storage.
fn run_id() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{secs}")
}

fn probe(base: &str) -> Result<(), String> {
    println!("probe {base}");
    let (r, ms) = send(&ehttp::Request::get(format!("{base}/")))?;
    show("GET /", ms, format!("HTTP {}", r.status));
    let req = api::authenticate_device(base, "defaultkey", "probe-device-0001", false);
    let (r, ms) = send(&req)?;
    let rejected = r.status == 401;
    show("sign-in with the default server key", ms, format!("HTTP {} ({})", r.status, if rejected { "rejected, good" } else { "NOT rejected" }));
    if !rejected {
        return Err("the placeholder server key is still accepted".into());
    }
    Ok(())
}

fn e1(cfg: &config::Config, device: &str) -> Result<api::Session, String> {
    println!("E1 sessions (device {device})");
    let create = api::authenticate_device(&cfg.base_url, &cfg.server_key, device, true);
    let (r, ms) = send(&create)?;
    let s1 = api::parse_session(&r).map_err(|e| e.to_string())?;
    let (claims, uid) = api::claims(&s1.token).ok_or("cannot read the session token claims")?;
    show("authenticate (create=true)", ms, format!("created={} token={}B lifetime={}s uid={}...", s1.created, s1.token.len(), claims.exp - claims.iat, &uid[..8.min(uid.len())]));

    let (r, ms) = send(&api::authenticate_device(&cfg.base_url, &cfg.server_key, device, false))?;
    let s2 = api::parse_session(&r).map_err(|e| e.to_string())?;
    show("authenticate again (create=false)", ms, format!("created={} (must be false)", s2.created));
    if s2.created {
        return Err("a second sign-in of the same device created a new account".into());
    }

    let (r, ms) = send(&api::refresh_session(&cfg.base_url, &cfg.server_key, &s2.refresh_token))?;
    let s3 = api::parse_session(&r).map_err(|e| e.to_string())?;
    let (c3, _) = api::claims(&s3.token).ok_or("cannot read the refreshed token claims")?;
    show("refresh session", ms, format!("new token lifetime={}s", c3.exp - c3.iat));

    // A wrong refresh token must fail (never print the server's reply body for a success path).
    let (r, ms) = send(&api::refresh_session(&cfg.base_url, &cfg.server_key, "not-a-token"))?;
    show("refresh with a garbage token", ms, format!("HTTP {} (must be an error)", r.status));
    if (200..300).contains(&r.status) {
        return Err("a garbage refresh token was accepted".into());
    }
    Ok(s3)
}

fn e2(cfg: &config::Config, session: &api::Session) -> Result<(), String> {
    println!("E2 storage");
    let (_, uid) = api::claims(&session.token).ok_or("cannot read the user id")?;
    let key = format!("slot-{}", run_id());
    let doc = r#"{"level":3,"inventory":["sword","potion"],"hp":42}"#;

    let (r, ms) = send(&api::storage_write(&cfg.base_url, &session.token, "spike_saves", &key, doc, Some("*")))?;
    let v1 = api::parse_write_ack(&r).map_err(|e| e.to_string())?;
    show("write (create only, version=*)", ms, format!("ack version={}...", &v1[..8.min(v1.len())]));

    let (r, ms) = send(&api::storage_read(&cfg.base_url, &session.token, "spike_saves", &key, &uid))?;
    let got = api::parse_read(&r).map_err(|e| e.to_string())?.ok_or("the object just written was not found")?;
    // Nakama stores the value as normalized JSON (Postgres jsonb): key order and whitespace are NOT preserved, so the
    // comparison is on the parsed document and the exact-text difference is itself a finding.
    let same_text = got.0 == doc;
    let same_json = serde_json::from_str::<serde_json::Value>(&got.0).ok() == serde_json::from_str::<serde_json::Value>(doc).ok();
    show("read it back", ms, format!("{} bytes, version matches: {}, same JSON: {}, byte-identical text: {}", got.0.len(), got.1 == v1, same_json, same_text));
    if !same_json {
        return Err(format!("round trip changed the document content: {}", got.0));
    }

    let (r, ms) = send(&api::storage_write(&cfg.base_url, &session.token, "spike_saves", &key, r#"{"level":4}"#, Some(&v1)))?;
    let v2 = api::parse_write_ack(&r).map_err(|e| e.to_string())?;
    show("write with the current version", ms, format!("ack new version differs: {}", v2 != v1));

    let (r, ms) = send(&api::storage_write(&cfg.base_url, &session.token, "spike_saves", &key, r#"{"level":99}"#, Some(&v1)))?;
    let conflict = ApiError { status: r.status, body: r.text().unwrap_or("").to_string() };
    show("write with the STALE version (conflict)", ms, &conflict);
    if (200..300).contains(&conflict.status) {
        return Err("a stale-version write was accepted: optimistic locking does not work as assumed".into());
    }

    let (r, ms) = send(&api::storage_write(&cfg.base_url, &session.token, "spike_saves", &key, r#"{"level":5}"#, Some("*")))?;
    show("create-only write on an existing key", ms, format!("HTTP {} (must be an error)", r.status));
    if (200..300).contains(&r.status) {
        return Err("version=* overwrote an existing object".into());
    }
    Ok(())
}

fn main() {
    let cmd = std::env::args().nth(1).unwrap_or_else(|| "all".to_string());
    let base = std::env::var("NAKAMA_URL").unwrap_or_else(|_| config::DEFAULT_URL.to_string());
    let base = base.trim_end_matches('/').to_string();

    let result = (|| -> Result<(), String> {
        match cmd.as_str() {
            "probe" => probe(&base),
            "e1" | "e2" | "all" => {
                if cmd == "all" {
                    probe(&base)?;
                }
                let cfg = config::Config::load()?;
                let device = format!("spike-device-{}", run_id());
                let session = e1(&cfg, &device)?;
                if cmd != "e1" {
                    e2(&cfg, &session)?;
                }
                Ok(())
            }
            other => Err(format!("unknown experiment '{other}' (probe | e1 | e2 | all)")),
        }
    })();
    if let Err(e) = result {
        eprintln!("FAILED: {e}");
        std::process::exit(1);
    }
    println!("ok");
}

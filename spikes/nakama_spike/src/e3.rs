//! E3: a leaderboard created by a server module (`server/spike_leaderboard.lua`), a client submitting a score and listing the top
//! records. Needs the module deployed to the Nakama modules directory first.

use nakama_spike::config::Config;
use nakama_spike::protocol::rest as api;

use crate::{run_id, send, show};

pub fn run(cfg: &Config) -> Result<(), String> {
    println!("E3 leaderboard (server-created, client-submitted)");
    let run = run_id();
    let mut users = Vec::new();
    for (i, score) in [120i64, 450, 300].into_iter().enumerate() {
        let (r, _) = send(&api::authenticate_device(&cfg.base_url, &cfg.server_key, &format!("spike-lb-{run}-{i}"), true))?;
        let s = api::parse_session(&r).map_err(|e| e.to_string())?;
        let (r, took) = send(&api::leaderboard_submit(&cfg.base_url, &s.token, "spike_best", score))?;
        let (status, body) = api::raw(&r);
        let detail = if status == 200 { String::new() } else { format!(": {}", body.chars().take(200).collect::<String>()) };
        show(&format!("user {i} submits score {score}"), took, format!("HTTP {status}{detail}"));
        if status != 200 {
            return Err(if status == 404 {
                "leaderboard spike_best does not exist: deploy server/spike_leaderboard.lua and restart Nakama".into()
            } else {
                format!("submit failed: HTTP {status}")
            });
        }
        users.push(s);
    }
    let (r, took) = send(&api::leaderboard_top(&cfg.base_url, &users[0].token, "spike_best", 10))?;
    let top = api::parse_leaderboard(&r).map_err(|e| e.to_string())?;
    show("top 10", took, format!("{} record(s); scores best-first: {:?}", top.len(), top.iter().map(|t| t.1).collect::<Vec<_>>()));
    // A client may submit any number: a score is trusted unless the leaderboard is authoritative (known, recorded not measured).
    let (r, _) = send(&api::leaderboard_submit(&cfg.base_url, &users[0].token, "spike_best", 999_999_999))?;
    show("client submits an absurd score (trusted?)", 0.0, format!("HTTP {}", r.status));
    Ok(())
}

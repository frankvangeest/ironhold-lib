//! `ironhold validate` checks for `backend.ron` and the online actions (`planning/features/backend_abstraction.md`, v1).
//!
//! Kept apart from `validate.rs` (which is large) and written against plain values so it does not need that module's
//! private `LoadedProject`.

use std::collections::HashSet;
use std::path::Path;

use ironhold_core::schema::backend::{is_valid_backend_id, BackendConfig, MAX_LEADERBOARD_ROWS};
use ironhold_core::schema::{Action, ProjectConfig};

use super::utils::{find_project_ron, ron_from_str};

pub struct Finding {
    pub source_file: String,
    pub message: String,
    pub kind: &'static str,
}

fn is_online_action(a: &Action) -> bool {
    matches!(a, Action::SignIn | Action::SignOut | Action::SubmitScore { .. } | Action::FetchLeaderboard { .. })
}

/// `http://` is only acceptable for a machine on the developer's own network.
fn is_local_host(endpoint: &str) -> bool {
    let lower = endpoint.to_lowercase();
    let authority = lower.strip_prefix("http://").unwrap_or(&lower).split('/').next().unwrap_or("");
    // `[::1]:7350` keeps its brackets; `host:port` loses the port.
    let host = match authority.strip_prefix('[') {
        Some(v6) => format!("[{}", v6.split(']').next().unwrap_or("")) + "]",
        None => authority.split(':').next().unwrap_or("").to_string(),
    };
    let octets: Vec<u8> = host.split('.').filter_map(|p| p.parse().ok()).collect();
    host == "localhost"
        || host == "[::1]"
        || host.ends_with(".local")
        || host.ends_with(".home")
        || (octets.len() == 4
            && (octets[0] == 127
                || octets[0] == 10
                || (octets[0] == 169 && octets[1] == 254)
                || (octets[0] == 192 && octets[1] == 168)
                || (octets[0] == 172 && (16..=31).contains(&octets[1]))))
}

/// Hard errors: a config the engine would reject or an action that can never work.
pub fn errors(
    backend_source: &str,
    backend: Option<&BackendConfig>,
    project_id: Option<&str>,
    actions: &[(String, Action)],
) -> Vec<Finding> {
    let mut out = Vec::new();
    if let Some(cfg) = backend {
        for problem in cfg.problems() {
            out.push(Finding { source_file: backend_source.to_string(), message: problem, kind: "backend_config" });
        }
        // The namespace actually used on a server: an omitted game_id falls back to project_id, which must be valid too.
        match cfg.effective_game_id(project_id) {
            None => out.push(Finding {
                source_file: backend_source.to_string(),
                message: "no game_id here and no project_id in .project.ron: the project has no namespace on a shared server".into(),
                kind: "backend_config",
            }),
            Some(id) if cfg.game_id.is_none() && !is_valid_backend_id(&id) => out.push(Finding {
                source_file: backend_source.to_string(),
                message: format!(
                    "game_id is omitted, so project_id {id:?} is used, and it must be 1-32 characters of [a-z0-9_]; set game_id explicitly"
                ),
                kind: "backend_config",
            }),
            Some(_) => {}
        }
        if let Some(ep) = cfg.endpoint.as_deref() {
            if ep.starts_with("http://") && !is_local_host(ep) {
                out.push(Finding {
                    source_file: backend_source.to_string(),
                    message: format!(
                        "endpoint {ep:?} uses plain http to a host outside localhost/LAN; a hosted page cannot reach it (mixed \
                         content) and traffic is unencrypted - use https://"
                    ),
                    kind: "backend_endpoint_http",
                });
            }
        }
    }
    for (src, action) in actions {
        let leaderboard = match action {
            Action::SubmitScore { leaderboard, .. } | Action::FetchLeaderboard { leaderboard, .. } => leaderboard,
            _ => continue,
        };
        if let Action::FetchLeaderboard { count, .. } = action {
            if *count == 0 || *count > MAX_LEADERBOARD_ROWS {
                out.push(Finding {
                    source_file: src.clone(),
                    message: format!("FetchLeaderboard(leaderboard: {leaderboard:?}): count {count} must be 1-{MAX_LEADERBOARD_ROWS}"),
                    kind: "leaderboard_count_range",
                });
            }
        }
        // Without a backend.ron the strict warning below covers it; here the file exists and the name is not in it.
        if let Some(cfg) = backend {
            if cfg.leaderboard(leaderboard).is_none() {
                out.push(Finding {
                    source_file: src.clone(),
                    message: format!(
                        "leaderboard {leaderboard:?} is not declared in {backend_source} (declared: {})",
                        if cfg.leaderboards.is_empty() {
                            "none".to_string()
                        } else {
                            cfg.leaderboards.iter().map(|l| l.name.as_str()).collect::<Vec<_>>().join(", ")
                        }
                    ),
                    kind: "undeclared_leaderboard",
                });
            }
        }
    }
    out
}

/// `--strict` warnings: things that work but are probably not what the designer meant.
pub fn strict_warnings(
    project_dir: &Path,
    project_config: Option<&ProjectConfig>,
    backend_source: &str,
    backend: Option<&BackendConfig>,
    actions: &[(String, Action)],
) -> Vec<Finding> {
    let mut out = Vec::new();

    if backend.is_none() && project_config.is_some_and(|c| c.backend_path.is_none()) {
        let mut seen = HashSet::new();
        for (src, action) in actions.iter().filter(|(_, a)| is_online_action(a)) {
            if seen.insert(src.clone()) {
                out.push(Finding {
                    source_file: src.clone(),
                    message: format!(
                        "uses the online action {action:?} but the project has no backend_path in .project.ron: it will only ever \
                         answer backend.offline"
                    ),
                    kind: "online_action_without_backend",
                });
            }
        }
    }

    if let Some(cfg) = backend {
        if let Some(ep) = cfg.endpoint.as_deref() {
            if ep.starts_with("http://") && is_local_host(ep) {
                out.push(Finding {
                    source_file: backend_source.to_string(),
                    message: format!("endpoint {ep:?} is plain http (fine on a LAN; a hosted https page cannot use it)"),
                    kind: "backend_plain_http",
                });
            }
        }
    }

    // `backend.*` and `leaderboard.*` belong to the engine: a rule writing them is overwritten at the next reply.
    let mut reserved_seen = HashSet::new();
    for (src, action) in actions {
        if let Action::SetVariable(k, _) | Action::IncrementVariable(k, _) = action {
            if (k.starts_with("backend.") || k.starts_with("leaderboard.")) && reserved_seen.insert((src.clone(), k.clone())) {
                out.push(Finding {
                    source_file: src.clone(),
                    message: format!("writes the engine-owned variable {k:?}; the engine overwrites it at the next backend reply"),
                    kind: "reserved_variable_write",
                });
            }
        }
    }

    // A score variable nothing ever writes submits whatever the player's save happened to hold, or fails every time.
    let written: HashSet<&str> = actions
        .iter()
        .filter_map(|(_, a)| match a {
            Action::SetVariable(k, _) | Action::IncrementVariable(k, _) => Some(k.as_str()),
            _ => None,
        })
        .collect();
    let mut reported = HashSet::new();
    for (src, action) in actions {
        if let Action::SubmitScore { score_var, .. } = action {
            if !written.contains(score_var.as_str()) && reported.insert(score_var.clone()) {
                out.push(Finding {
                    source_file: src.clone(),
                    message: format!("SubmitScore reads variable {score_var:?}, but no SetVariable/IncrementVariable in this project writes it"),
                    kind: "score_var_never_written",
                });
            }
        }
    }

    // game_id is the namespace on a shared server: two projects with the same one would share leaderboards and saves.
    if let (Some(cfg), Some(pc)) = (backend, project_config) {
        let mine = cfg.effective_game_id(pc.project_id.as_deref());
        if let (Some(mine), Some(parent)) = (mine, project_dir.parent()) {
            for other in sibling_game_ids(parent, project_dir) {
                if other.1 == mine {
                    out.push(Finding {
                        source_file: backend_source.to_string(),
                        message: format!("game_id {mine:?} is also used by project {:?}; ids must be unique per server", other.0),
                        kind: "duplicate_game_id",
                    });
                }
            }
        }
    }
    out
}

/// `(project dir name, effective game_id)` of every other project next to this one that declares a backend.
fn sibling_game_ids(parent: &Path, this: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(parent) else { return out };
    for entry in entries.flatten() {
        let dir = entry.path();
        // Same folder under a different spelling (Windows is case-insensitive) must not count as "another project".
        let same = dir.file_name().map(|n| n.to_string_lossy().to_lowercase())
            == this.file_name().map(|n| n.to_string_lossy().to_lowercase());
        if !dir.is_dir() || dir == this || same {
            continue;
        }
        let Some(ron_name) = find_project_ron(&dir) else { continue };
        let Ok(text) = std::fs::read_to_string(dir.join(&ron_name)) else { continue };
        let Ok(pc) = ron_from_str::<ProjectConfig>(&text) else { continue };
        let Some(bpath) = pc.backend_path.as_deref() else { continue };
        let Ok(btext) = std::fs::read_to_string(dir.join(bpath)) else { continue };
        let Ok(bc) = ron_from_str::<BackendConfig>(&btext) else { continue };
        if let Some(id) = bc.game_id.or(pc.project_id) {
            out.push((entry.file_name().to_string_lossy().into_owned(), id));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_http_is_only_local() {
        for ok in [
            "http://localhost:7350", "http://LocalHost:7350", "http://127.0.0.1:7350", "http://192.168.178.106:7350",
            "http://nakama.home", "http://10.1.2.3", "http://[::1]:7350", "http://169.254.1.2",
        ] {
            assert!(is_local_host(ok), "{ok}");
        }
        for bad in ["http://example.org", "http://8.8.8.8:7350", "http://172.32.0.1"] {
            assert!(!is_local_host(bad), "{bad}");
        }
    }
}

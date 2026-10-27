//! `ironhold validate` checks for `backend.ron` and the online actions (`planning/features/backend_abstraction.md`, v1).
//!
//! Kept apart from `validate.rs` (which is large) and written against plain values so it does not need that module's
//! private `LoadedProject`.

use std::collections::HashSet;
use std::path::Path;

use ironhold_core::schema::backend::{BackendConfig, MAX_LEADERBOARD_ROWS};
use ironhold_core::schema::{Action, ProjectConfig};

use super::utils::{find_project_ron, ron_from_str};

pub struct Finding {
    pub source_file: String,
    pub message: String,
    pub kind: &'static str,
}

fn is_online_action(a: &Action) -> bool {
    matches!(a, Action::SignIn | Action::SignOut | Action::SubmitScore { .. } | Action::ShowLeaderboard { .. })
}

/// `http://` is only acceptable for a machine on the developer's own network.
fn is_local_host(endpoint: &str) -> bool {
    let host = endpoint
        .strip_prefix("http://")
        .unwrap_or(endpoint)
        .split(['/', ':'])
        .next()
        .unwrap_or("");
    let octets: Vec<u8> = host.split('.').filter_map(|p| p.parse().ok()).collect();
    host == "localhost"
        || host.ends_with(".local")
        || host.ends_with(".home")
        || (octets.len() == 4
            && (octets[0] == 127
                || octets[0] == 10
                || (octets[0] == 192 && octets[1] == 168)
                || (octets[0] == 172 && (16..=31).contains(&octets[1]))))
}

/// Hard errors: a config the engine would reject or an action that can never work.
pub fn errors(backend_source: &str, backend: Option<&BackendConfig>, actions: &[(String, Action)]) -> Vec<Finding> {
    let mut out = Vec::new();
    if let Some(cfg) = backend {
        for problem in cfg.problems() {
            out.push(Finding { source_file: backend_source.to_string(), message: problem, kind: "backend_config" });
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
            Action::SubmitScore { leaderboard, .. } | Action::ShowLeaderboard { leaderboard, .. } => leaderboard,
            _ => continue,
        };
        if let Action::ShowLeaderboard { count, .. } = action {
            if *count == 0 || *count > MAX_LEADERBOARD_ROWS {
                out.push(Finding {
                    source_file: src.clone(),
                    message: format!("ShowLeaderboard(leaderboard: {leaderboard:?}): count {count} must be 1-{MAX_LEADERBOARD_ROWS}"),
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
        let mine = cfg.game_id.clone().or_else(|| pc.project_id.clone());
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
        if !dir.is_dir() || dir == this {
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
        for ok in ["http://localhost:7350", "http://127.0.0.1:7350", "http://192.168.178.106:7350", "http://nakama.home", "http://10.1.2.3"] {
            assert!(is_local_host(ok), "{ok}");
        }
        for bad in ["http://example.org", "http://8.8.8.8:7350", "http://172.32.0.1"] {
            assert!(!is_local_host(bad), "{bad}");
        }
    }
}

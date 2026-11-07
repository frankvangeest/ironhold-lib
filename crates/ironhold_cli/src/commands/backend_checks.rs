//! `ironhold validate` checks for `backend.ron` and the online actions (`planning/features/backend_abstraction.md`, v1).
//!
//! Kept apart from `validate.rs` (which is large) and written against plain values so it does not need that module's
//! private `LoadedProject`.

use std::collections::HashSet;
use std::path::Path;

use ironhold_core::schema::backend::{
    is_fake_endpoint, is_lobby_event, is_lobby_variable, is_valid_backend_id, BackendConfig, BackendFeature, MAX_LEADERBOARD_ROWS,
    MAX_LOBBY_PLAYERS, MAX_LOBBY_ROWS, MIN_LOBBY_PLAYERS, RESERVED_VARIABLE_PREFIXES,
};
use ironhold_core::schema::{Action, ProjectConfig};

use super::utils::{find_project_ron, ron_from_str};

pub struct Finding {
    pub source_file: String,
    pub message: String,
    pub kind: &'static str,
}

fn is_online_action(a: &Action) -> bool {
    matches!(a, Action::SignIn | Action::SignOut | Action::SubmitScore { .. } | Action::FetchLeaderboard { .. }) || is_lobby_action(a)
}

fn is_lobby_action(a: &Action) -> bool {
    matches!(a, Action::ListLobbies | Action::CreateLobby { .. } | Action::JoinLobby { .. } | Action::LeaveLobby | Action::StartLobby)
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
    out.extend(lobby_errors(backend_source, backend, actions));
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

/// Lobby actions that can never work: a row or a size outside the engine's range, a name the server would refuse, or a project that
/// never declared the `Lobbies` feature (the runtime answers `lobby.<op>_failed:not_enabled` for those).
fn lobby_errors(backend_source: &str, backend: Option<&BackendConfig>, actions: &[(String, Action)]) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut undeclared_reported = HashSet::new();
    for (src, action) in actions.iter().filter(|(_, a)| is_lobby_action(a)) {
        let mut push = |message: String, kind: &'static str| out.push(Finding { source_file: src.clone(), message, kind });
        if let Action::JoinLobby { row } = action {
            if !(1..=MAX_LOBBY_ROWS).contains(row) {
                push(format!("JoinLobby(row: {row}): row must be 1-{MAX_LOBBY_ROWS} (the list shows at most {MAX_LOBBY_ROWS} rows)"), "lobby_row_range");
            }
        }
        if let Action::CreateLobby { name, max_players } = action {
            if !(MIN_LOBBY_PLAYERS..=MAX_LOBBY_PLAYERS).contains(max_players) {
                push(format!("CreateLobby(max_players: {max_players}): must be {MIN_LOBBY_PLAYERS}-{MAX_LOBBY_PLAYERS} players"), "lobby_max_players_range");
            }
            if let Some(cfg) = backend {
                if !cfg.lobby_names.contains(name) {
                    let declared = if cfg.lobby_names.is_empty() { "none".to_string() } else { cfg.lobby_names.join(", ") };
                    push(
                        format!("CreateLobby(name: {name:?}) is not in lobby_names of {backend_source} (declared: {declared}); the server refuses any other name"),
                        "lobby_name_not_declared",
                    );
                }
            }
        }
        // Without a backend.ron the strict `online_action_without_backend` warning covers it; here the file exists.
        if let Some(cfg) = backend {
            if !cfg.features.contains(&BackendFeature::Lobbies) && undeclared_reported.insert(src.clone()) {
                out.push(Finding {
                    source_file: src.clone(),
                    message: format!("{action:?} needs `Lobbies` in `features` of {backend_source}; without it every lobby action answers lobby.<op>_failed:not_enabled"),
                    kind: "lobby_feature_not_declared",
                });
            }
        }
    }
    out
}

/// `--strict` warnings about the lobby vocabulary. `events` are the event names rules are bound to and `bound_vars` the variables Labels
/// are bound to, each with the file they came from. The vocabulary is closed, so a name outside it is a typo, or a row the list never shows.
pub fn lobby_vocabulary_warnings(events: &[(String, String)], bound_vars: &[(String, String)]) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (src, event) in events {
        if event.starts_with("lobby.") && !is_lobby_event(event) && seen.insert((src.clone(), event.clone())) {
            out.push(Finding {
                source_file: src.clone(),
                message: format!("bound to the event {event:?}, which the engine never emits for lobbies (a typo? see docs/20_data_formats.md, Lobbies)"),
                kind: "unknown_lobby_event",
            });
        }
    }
    for (src, var) in bound_vars {
        if var.starts_with("lobby.") && !is_lobby_variable(var) && seen.insert((src.clone(), var.clone())) {
            let kind = if is_row_beyond_the_list(var) { "lobby_row_binding_out_of_range" } else { "unknown_lobby_variable" };
            let why = if kind == "unknown_lobby_variable" {
                "the engine never writes it (a typo?)"
            } else {
                "the list shows at most 10 rows and a lobby holds at most 8 players, so it is always empty"
            };
            out.push(Finding { source_file: src.clone(), message: format!("a Label is bound to {var:?}, but {why}"), kind });
        }
    }
    out
}

/// `lobby.11.name`, `lobby.member.9.name`: the right shape with an index past the end.
fn is_row_beyond_the_list(var: &str) -> bool {
    let parts: Vec<&str> = var.trim_start_matches("lobby.").split('.').collect();
    match parts.as_slice() {
        ["member", n, "name"] => n.parse::<u32>().is_ok_and(|n| n > MAX_LOBBY_PLAYERS),
        [n, _] => n.parse::<u32>().is_ok_and(|n| n > MAX_LOBBY_ROWS),
        _ => false,
    }
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
        if let Some(key) = cfg.server_key.as_deref() {
            let ep = cfg.endpoint.as_deref();
            if ep.is_none_or(is_fake_endpoint) {
                out.push(Finding {
                    source_file: backend_source.to_string(),
                    message: "server_key is set but the endpoint is fake:// or missing, so the key is never used".into(),
                    kind: "backend_server_key_unused",
                });
            } else if key == "defaultkey" && ep.is_some_and(|e| e.starts_with("https://")) {
                out.push(Finding {
                    source_file: backend_source.to_string(),
                    message: "server_key is Nakama's built-in default \"defaultkey\" on a public https endpoint: set a key of your own in the server's socket.server_key"
                        .into(),
                    kind: "backend_default_server_key",
                });
            }
        }
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

    // A join resolves against the last list: with no `ListLobbies` anywhere, every `JoinLobby` answers `lobby.join_failed:no_lobby`.
    if !actions.iter().any(|(_, a)| matches!(a, Action::ListLobbies)) {
        let mut seen = HashSet::new();
        for (src, _) in actions.iter().filter(|(_, a)| matches!(a, Action::JoinLobby { .. })) {
            if seen.insert(src.clone()) {
                out.push(Finding {
                    source_file: src.clone(),
                    message: "uses JoinLobby but no rule in this project runs ListLobbies, so no row is ever filled".into(),
                    kind: "join_without_list",
                });
            }
        }
    }

    // `backend.*`, `leaderboard.*` and `lobby.*` belong to the engine: a rule writing them is overwritten at the next reply.
    let mut reserved_seen = HashSet::new();
    for (src, action) in actions {
        if let Action::SetVariable(k, _) | Action::IncrementVariable(k, _) = action {
            if RESERVED_VARIABLE_PREFIXES.iter().any(|p| k.starts_with(p)) && reserved_seen.insert((src.clone(), k.clone())) {
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

    fn lobby_config(extra: &str) -> BackendConfig {
        ron::from_str(&format!(r#"BackendConfig(schema_version: 1, endpoint: Some("fake://"), {extra})"#)).unwrap()
    }

    fn kinds(findings: Vec<Finding>) -> Vec<&'static str> {
        findings.into_iter().map(|f| f.kind).collect()
    }

    fn acts(actions: Vec<Action>) -> Vec<(String, Action)> {
        actions.into_iter().map(|a| ("logic/state_machine.ron".to_string(), a)).collect()
    }

    #[test]
    fn lobby_actions_need_the_feature_valid_numbers_and_a_declared_name() {
        let ok = lobby_config(r#"features: [Lobbies], lobby_names: ["Demo lobby"]"#);
        let good = acts(vec![
            Action::ListLobbies,
            Action::CreateLobby { name: "Demo lobby".into(), max_players: 4 },
            Action::JoinLobby { row: 10 },
            Action::LeaveLobby,
            Action::StartLobby,
        ]);
        assert!(kinds(errors("backend.ron", Some(&ok), Some("p"), &good)).is_empty());

        let bad = acts(vec![
            Action::JoinLobby { row: 0 },
            Action::JoinLobby { row: 11 },
            Action::CreateLobby { name: "Demo lobby".into(), max_players: 1 },
            Action::CreateLobby { name: "Demo lobby".into(), max_players: 9 },
            Action::CreateLobby { name: "Other room".into(), max_players: 4 },
        ]);
        assert_eq!(
            kinds(errors("backend.ron", Some(&ok), Some("p"), &bad)),
            ["lobby_row_range", "lobby_row_range", "lobby_max_players_range", "lobby_max_players_range", "lobby_name_not_declared"]
        );
        // Both problems of one action are reported, not only the first.
        let both = acts(vec![Action::CreateLobby { name: "Nope".into(), max_players: 20 }]);
        assert_eq!(kinds(errors("backend.ron", Some(&ok), Some("p"), &both)), ["lobby_max_players_range", "lobby_name_not_declared"]);

        // A backend.ron that never declared `Lobbies`: one report per file, however many lobby actions it holds.
        let no_feature = lobby_config(r#"lobby_names: ["Demo lobby"]"#);
        assert_eq!(kinds(errors("backend.ron", Some(&no_feature), Some("p"), &good)), ["lobby_feature_not_declared"]);
    }

    #[test]
    fn a_join_without_any_list_and_a_rule_writing_a_lobby_variable_are_flagged() {
        let cfg = lobby_config(r#"features: [Lobbies], lobby_names: ["Demo lobby"]"#);
        let warn = |actions: Vec<Action>| kinds(strict_warnings(Path::new("."), None, "backend.ron", Some(&cfg), &acts(actions)));
        assert_eq!(warn(vec![Action::JoinLobby { row: 1 }]), ["join_without_list"]);
        assert!(warn(vec![Action::ListLobbies, Action::JoinLobby { row: 1 }]).is_empty());
        assert_eq!(warn(vec![Action::SetVariable("lobby.count".into(), "5".into())]), ["reserved_variable_write"]);
        assert_eq!(warn(vec![Action::SetVariable("backend.status".into(), "x".into())]), ["reserved_variable_write"]);
        assert!(warn(vec![Action::SetVariable("lobbying".into(), "x".into())]).is_empty(), "only the `lobby.` prefix is reserved");
    }

    #[test]
    fn a_typo_in_the_lobby_vocabulary_and_a_row_past_the_end_are_flagged() {
        let ev = |names: &[&str]| -> Vec<(String, String)> {
            names.iter().map(|n| ("logic/state_machine.ron".to_string(), n.to_string())).collect()
        };
        let var = |names: &[&str]| -> Vec<(String, String)> {
            names.iter().map(|n| ("scenes/main.scene.ron".to_string(), n.to_string())).collect()
        };
        // Fine: every name the engine emits, and events outside the lobby prefix are not this check's business.
        let fine_events = ev(&["lobby.joined", "lobby.join_failed:full", "lobby.row_filled:3", "ui.button_pressed:x"]);
        assert!(lobby_vocabulary_warnings(&fine_events, &var(&["lobby.1.name", "lobby.current.players"])).is_empty());
        assert_eq!(kinds(lobby_vocabulary_warnings(&ev(&["lobby.joind", "lobby.joind"]), &[])), ["unknown_lobby_event"], "one report per file and name");
        assert_eq!(kinds(lobby_vocabulary_warnings(&ev(&["lobby.join_failed:fulll"]), &[])), ["unknown_lobby_event"]);
        assert_eq!(kinds(lobby_vocabulary_warnings(&[], &var(&["lobby.current.nmae"]))), ["unknown_lobby_variable"]);
        assert_eq!(
            kinds(lobby_vocabulary_warnings(&[], &var(&["lobby.11.name", "lobby.member.9.name"]))),
            ["lobby_row_binding_out_of_range", "lobby_row_binding_out_of_range"]
        );
    }

    #[test]
    fn a_server_key_that_is_unused_or_the_public_default_is_flagged() {
        let kinds = |ron: &str| -> Vec<&'static str> {
            let cfg: BackendConfig = ron::from_str(ron).unwrap();
            strict_warnings(Path::new("."), None, "backend.ron", Some(&cfg), &[]).into_iter().map(|f| f.kind).collect()
        };
        assert_eq!(kinds(r#"BackendConfig(schema_version: 1, endpoint: Some("fake://"), server_key: Some("k"))"#), ["backend_server_key_unused"]);
        assert_eq!(kinds(r#"BackendConfig(schema_version: 1, server_key: Some("k"))"#), ["backend_server_key_unused"]);
        assert_eq!(
            kinds(r#"BackendConfig(schema_version: 1, endpoint: Some("https://play.example.org"), server_key: Some("defaultkey"))"#),
            ["backend_default_server_key"]
        );
        assert!(kinds(r#"BackendConfig(schema_version: 1, endpoint: Some("https://play.example.org"), server_key: Some("mine"))"#).is_empty());
        // The default key is fine on a LAN server; only the plain-http note remains.
        assert_eq!(
            kinds(r#"BackendConfig(schema_version: 1, endpoint: Some("http://localhost:7350"), server_key: Some("defaultkey"))"#),
            ["backend_plain_http"]
        );
    }
}

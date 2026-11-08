//! Contract cases for the six `ironhold_lobby_*` RPCs (planning/features/backend_lobbies_server.md). Lua is only exercised here, so
//! this file is the real gate for `ironhold.lua`'s lobby section. Raw HTTP, so the cases can send what an honest adapter never would.
//! The module under test runs with `ironhold_test_overrides.lua` (lobby expiry 6 s); games `alpha` and `gamma` have lobbies, `beta` has
//! none.

use std::time::Duration;

use serde_json::{json, Value};

use ironhold_core::runtime::backend::{Backend, BackendReply, BackendRequest, RequestKind};

use super::{adapter, ask, basic, fresh_device_id, fresh_secret, raw, raw_session};

/// The lobby expiry the test module runs with (`modules/ironhold_test_overrides.lua`).
const EXPIRY: u64 = 6;

struct Player {
    token: String,
    id: String,
    name: String,
}

fn player(game: &str) -> Player {
    let (token, _) = raw_session(&fresh_device_id(game), "").expect("a device signs in");
    account(&token)
}

fn account(token: &str) -> Player {
    let (status, body) = raw("GET", "/v2/account", &format!("Bearer {token}"), "");
    assert_eq!(status, 200, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    Player {
        token: token.to_string(),
        id: v["user"]["id"].as_str().unwrap().to_string(),
        name: v["user"]["username"].as_str().unwrap().to_string(),
    }
}

/// Status and body of one RPC call (the body is string-encoded JSON, as Nakama's REST gateway wants it).
fn call_raw(p: &Player, rpc: &str, body: &str) -> (u16, String) {
    let wire = serde_json::to_string(body).unwrap();
    raw("POST", &format!("/v2/rpc/{rpc}"), &format!("Bearer {}", p.token), &wire)
}

/// The decoded reply of an RPC that must answer HTTP 200.
fn call(p: &Player, rpc: &str, body: Value) -> Value {
    let (status, text) = call_raw(p, rpc, &body.to_string());
    assert_eq!(status, 200, "{rpc} {body}: {text}");
    let outer: Value = serde_json::from_str(&text).unwrap();
    serde_json::from_str(outer["payload"].as_str().expect("a payload")).unwrap()
}

fn create(p: &Player, name: &str, max: u32) -> Value {
    call(p, "ironhold_lobby_create", json!({ "name": name, "max_players": max }))
}
fn join(p: &Player, id: &str) -> Value {
    call(p, "ironhold_lobby_join", json!({ "lobby_id": id }))
}
fn heartbeat(p: &Player, id: &str) -> Value {
    call(p, "ironhold_lobby_heartbeat", json!({ "lobby_id": id }))
}
fn start(p: &Player, id: &str) -> Value {
    call(p, "ironhold_lobby_start", json!({ "lobby_id": id }))
}
fn leave(p: &Player) -> Value {
    call(p, "ironhold_lobby_leave", json!({}))
}
fn list(p: &Player) -> Vec<Value> {
    let v = call(p, "ironhold_lobby_list", json!({}));
    assert_eq!(v["ok"], true, "{v}");
    v["rows"].as_array().cloned().unwrap_or_default()
}

fn id_of(created: &Value) -> String {
    assert_eq!(created["ok"], true, "{created}");
    created["lobby"]["id"].as_str().unwrap().to_string()
}
fn members(v: &Value) -> Vec<String> {
    v["lobby"]["members"].as_array().unwrap().iter().map(|m| m.as_str().unwrap().to_string()).collect()
}
fn is_listed(p: &Player, id: &str) -> bool {
    list(p).iter().any(|r| r["id"] == id)
}
fn wait(secs: u64) {
    std::thread::sleep(Duration::from_secs(secs));
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_happy_path_create_list_join_heartbeat_start() {
    let (a, b) = (player("alpha"), player("alpha"));
    let made = create(&a, "Casual", 4);
    let id = id_of(&made);
    assert_eq!(made["lobby"]["host"], a.name.as_str());
    assert_eq!(made["lobby"]["players"], 1);
    assert_eq!(members(&made), vec![a.name.clone()]);
    assert!(is_listed(&b, &id), "another player sees the lobby");
    let joined = join(&b, &id);
    assert_eq!(joined["ok"], true, "{joined}");
    assert_eq!(members(&joined), vec![a.name.clone(), b.name.clone()], "host first, join order");
    assert!(joined["lobby"]["version"].as_u64().unwrap() > made["lobby"]["version"].as_u64().unwrap());
    let hb = heartbeat(&a, &id);
    assert_eq!(members(&hb), vec![a.name.clone(), b.name.clone()]);
    assert_eq!(start(&b, &id)["reason"], "not_host");
    let started = start(&a, &id);
    assert_eq!(started["ok"], true, "{started}");
    assert_eq!(started["lobby"]["started"], true);
    assert_eq!(heartbeat(&b, &id)["lobby"]["started"], true, "a member learns of the start from its next heartbeat");
    assert_eq!(start(&a, &id)["reason"], "started");
    assert!(!is_listed(&b, &id), "a started lobby is no longer listed");
    assert_eq!(join(&player("alpha"), &id)["reason"], "started");
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_start_needs_two_players_and_a_host() {
    let (a, b) = (player("alpha"), player("alpha"));
    let id = id_of(&create(&a, "Casual", 4));
    assert_eq!(start(&a, &id)["reason"], "too_few_players");
    assert_eq!(start(&b, &id)["reason"], "not_in_lobby", "a stranger is not in the lobby");
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_rpcs_answer_not_enabled_for_a_game_without_lobbies() {
    let p = player("beta");
    for rpc in ["list", "create", "join", "leave", "heartbeat", "start"] {
        let v = call(&p, &format!("ironhold_lobby_{rpc}"), json!({ "name": "Casual", "max_players": 4, "lobby_id": "0".repeat(32) }));
        assert_eq!(v["reason"], "not_enabled", "{rpc}: {v}");
    }
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_create_validates_name_and_size() {
    let p = player("alpha");
    for bad in ["Not declared", "", "caf\u{e9}", &"x".repeat(33)] {
        assert_eq!(create(&p, bad, 4)["reason"], "server", "{bad:?}");
    }
    for bad in [json!(1), json!(9), json!(2.5), json!("4"), json!(null)] {
        let v = call(&p, "ironhold_lobby_create", json!({ "name": "Casual", "max_players": bad }));
        assert_eq!(v["reason"], "server", "{bad}");
    }
    // The two characters that need escaping in the generated Lua config are ordinary names.
    for name in ["Quote\"d", "Back\\slash"] {
        let v = create(&player("alpha"), name, 2);
        assert_eq!(v["ok"], true, "{name}: {v}");
        assert_eq!(v["lobby"]["name"], name);
    }
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_payload_fuzz_never_leaks_or_crashes() {
    let p = player("alpha");
    let id = id_of(&create(&p, "Casual", 4));
    let huge = format!(r#"{{"lobby_id":"{}"}}"#, "x".repeat(100_000));
    for rpc in ["list", "create", "join", "leave", "heartbeat", "start"] {
        let rpc = format!("ironhold_lobby_{rpc}");
        for body in ["not json", "[1,2]", "7", huge.as_str()] {
            let (status, text) = call_raw(&p, &rpc, body);
            assert_eq!(status, 200, "{rpc} {body:.20}: {text}");
            let outer: Value = serde_json::from_str(&text).unwrap();
            let reply: Value = serde_json::from_str(outer["payload"].as_str().unwrap()).unwrap();
            // `leave` has nothing to read and `list` ignores the body's content, but both must refuse non-objects.
            assert_eq!(reply["reason"], "server", "{rpc} {body:.20}: {reply}");
        }
    }
    for body in [json!({ "lobby_id": 5 }), json!({ "lobby_id": "short" }), json!({ "lobby_id": "A".repeat(32) })] {
        for rpc in ["join", "heartbeat", "start"] {
            assert_eq!(call(&p, &format!("ironhold_lobby_{rpc}"), body.clone())["reason"], "server", "{rpc} {body}");
        }
    }
    assert!(is_listed(&p, &id), "the junk changed nothing");
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_join_refuses_full_gone_and_other_games() {
    let (a, b, c) = (player("alpha"), player("alpha"), player("alpha"));
    let id = id_of(&create(&a, "Casual", 2));
    assert_eq!(join(&b, &id)["ok"], true);
    assert_eq!(join(&c, &id)["reason"], "full");
    assert_eq!(join(&c, &"0".repeat(32))["reason"], "gone");
    // Another game's player cannot reach this game's lobby, with the right id or any other.
    let g = player("gamma");
    assert_eq!(join(&g, &id)["reason"], "gone");
    assert_eq!(heartbeat(&g, &id)["reason"], "removed");
    assert_eq!(start(&g, &id)["reason"], "not_in_lobby");
    assert_eq!(leave(&g)["ok"], true, "leave is idempotent and touches only the caller's own game");
    assert!(!is_listed(&g, &id), "game gamma's list never shows alpha's lobbies");
    assert!(is_listed(&c, &id), "and alpha's lobby is untouched");
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_leave_member_then_host() {
    let (a, b) = (player("alpha"), player("alpha"));
    let id = id_of(&create(&a, "Casual", 4));
    join(&b, &id);
    assert_eq!(leave(&b)["ok"], true);
    assert_eq!(members(&heartbeat(&a, &id)), vec![a.name.clone()]);
    assert_eq!(heartbeat(&b, &id)["reason"], "removed", "the leaver is no longer a member");
    join(&b, &id);
    assert_eq!(leave(&a)["ok"], true);
    assert_eq!(heartbeat(&b, &id)["reason"], "host_left", "a host leaving closes the lobby for the others");
    assert!(!is_listed(&b, &id));
    assert_eq!(join(&player("alpha"), &id)["reason"], "gone", "a closed lobby cannot be joined");
    assert_eq!(leave(&a)["ok"], true, "leaving twice is fine");
    assert_eq!(leave(&player("alpha"))["ok"], true, "so is leaving with nothing");
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_a_late_leave_for_an_old_lobby_does_not_undo_a_newer_membership() {
    let (a, b) = (player("alpha"), player("alpha"));
    let old = id_of(&create(&a, "Casual", 4));
    assert_eq!(leave(&a)["ok"], true);
    let new = id_of(&create(&a, "Casual", 4));
    let late = call(&a, "ironhold_lobby_leave", json!({ "lobby_id": old }));
    assert_eq!(late["ok"], true);
    assert_eq!(join(&b, &new)["ok"], true, "the newer lobby is still open");
    assert_eq!(members(&heartbeat(&a, &new)).len(), 2, "and A is still in it");
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_two_players_race_for_the_last_slot() {
    for round in 0..5 {
        let (a, b, c) = (player("alpha"), player("alpha"), player("alpha"));
        let id = id_of(&create(&a, "Casual", 2));
        let (rb, rc) = std::thread::scope(|s| {
            let hb = s.spawn(|| join(&b, &id));
            let hc = s.spawn(|| join(&c, &id));
            (hb.join().unwrap(), hc.join().unwrap())
        });
        let wins = [&rb, &rc].iter().filter(|r| r["ok"] == true).count();
        let fulls = [&rb, &rc].iter().filter(|r| r["reason"] == "full").count();
        assert!((wins, fulls) == (1, 1), "round {round}: exactly one wins, the other hears `full`: {rb} / {rc}");
        assert_eq!(members(&heartbeat(&a, &id)).len(), 2, "round {round}");
    }
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_a_cross_join_neither_deadlocks_nor_half_writes() {
    for round in 0..3 {
        let (a, b) = (player("alpha"), player("alpha"));
        let (x, y) = (id_of(&create(&a, "Casual", 4)), id_of(&create(&b, "Casual", 4)));
        let (ra, rb) = std::thread::scope(|s| {
            let ha = s.spawn(|| join(&a, &y));
            let hb = s.spawn(|| join(&b, &x));
            (ha.join().unwrap(), hb.join().unwrap())
        });
        let wins = [&ra, &rb].iter().filter(|r| r["ok"] == true).count();
        assert!(wins == 1, "round {round}: the moves exclude each other (one wins, the other finds its target closed): {ra} / {rb}");
        // Whoever won is a member of exactly one open lobby, and nobody is a member of both.
        let in_x = heartbeat(&a, &x)["ok"] == true || heartbeat(&b, &x)["ok"] == true;
        let in_y = heartbeat(&a, &y)["ok"] == true || heartbeat(&b, &y)["ok"] == true;
        assert!(in_x != in_y, "round {round}: exactly one lobby is left open");
    }
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_after_a_reload_create_and_join_replace_the_old_membership() {
    let (a, b) = (player("alpha"), player("alpha"));
    let x = id_of(&create(&a, "Casual", 4));
    join(&b, &x);
    // The game forgot it was in X (a reload): creating Y replaces the membership.
    let y = id_of(&create(&a, "Casual", 4));
    assert_ne!(x, y);
    assert_eq!(heartbeat(&b, &x)["reason"], "host_left", "the others of the old lobby are told the host left");
    assert!(is_listed(&b, &y) && !is_listed(&b, &x));
    // Joining the lobby you are already in changes nothing (the version stays), including for the host.
    let before = heartbeat(&a, &y)["lobby"]["version"].as_u64().unwrap();
    let again = join(&a, &y);
    assert_eq!(again["ok"], true, "{again}");
    assert_eq!(again["lobby"]["version"].as_u64().unwrap(), before);
    // A joiner moving from one open lobby to another leaves the first.
    let c = player("alpha");
    let z = id_of(&create(&c, "Casual", 4));
    assert_eq!(join(&b, &z)["ok"], true);
    assert_eq!(join(&b, &y)["ok"], true, "B moves from Z to Y");
    assert_eq!(members(&heartbeat(&c, &z)), vec![c.name.clone()], "Z lost its member");
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_members_expire_without_a_heartbeat_but_not_inside_the_window() {
    let (a, b) = (player("alpha"), player("alpha"));
    let id = id_of(&create(&a, "Casual", 4));
    join(&b, &id);
    // A fresh joiner is not swept before its own first heartbeat (the join wrote its check-in).
    assert_eq!(members(&heartbeat(&a, &id)).len(), 2);
    // Negative case: B checks in inside the window and is kept while A checks in too.
    for _ in 0..4 {
        wait(2);
        assert_eq!(heartbeat(&b, &id)["ok"], true);
        assert_eq!(members(&heartbeat(&a, &id)).len(), 2, "a member inside the window is not removed");
    }
    // Now B goes silent: after the expiry A's heartbeat removes it, and B is told so.
    wait(EXPIRY + 1);
    assert_eq!(members(&heartbeat(&a, &id)), vec![a.name.clone()]);
    assert_eq!(heartbeat(&b, &id)["reason"], "removed");
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_a_silent_host_closes_the_lobby_and_is_told_removed_when_it_returns() {
    let (a, b) = (player("alpha"), player("alpha"));
    let id = id_of(&create(&a, "Casual", 4));
    join(&b, &id);
    wait(EXPIRY + 1);
    assert!(!is_listed(&b, &id), "a lobby whose host went silent is not listed");
    assert_eq!(heartbeat(&b, &id)["reason"], "host_left");
    assert_eq!(heartbeat(&a, &id)["reason"], "removed", "the returning host is told it was removed");
    assert_eq!(join(&player("alpha"), &id)["reason"], "gone");
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_a_finished_lobby_is_cleaned_up_without_touching_a_newer_one() {
    let (a, b, c) = (player("alpha"), player("alpha"), player("alpha"));
    let x = id_of(&create(&a, "Casual", 4));
    join(&b, &x);
    assert_eq!(start(&a, &x)["ok"], true);
    // A moves on to Y while X is finished; X's retention delete must not take A's check-in row (which now points at Y).
    let y = id_of(&create(&a, "Casual", 4));
    for _ in 0..4 {
        wait(2);
        assert_eq!(heartbeat(&a, &y)["ok"], true);
    }
    assert!(is_listed(&c, &y), "Y stays listed while X is deleted");
    assert!(is_listed(&c, &y));
    assert_eq!(join(&c, &x)["reason"], "gone", "X is gone");
    assert!(is_listed(&c, &y), "and still listed after the cleanup ran");
    assert_eq!(heartbeat(&a, &y)["ok"], true);
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_limit_reached_counts_only_live_lobbies() {
    // Game gamma has the 50-lobby cap to itself in this run. 50 hosts create one lobby each; the 51st is refused.
    let players: Vec<Player> = (0..51).map(|_| player("gamma")).collect();
    for p in &players[..50] {
        assert_eq!(create(p, "Casual", 2)["ok"], true);
    }
    assert_eq!(create(&players[50], "Casual", 2)["reason"], "limit_reached");
    // Nobody checks in: the hosts expire, the abandoned lobbies stop counting, and a new lobby can be made.
    wait(EXPIRY + 2);
    let v = create(&players[50], "Casual", 2);
    assert_eq!(v["ok"], true, "{v}");
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_collections_are_closed_to_clients() {
    let p = player("alpha");
    let bearer = format!("Bearer {}", p.token);
    for kind in ["lobbies", "lobby_hb", "lobby_of"] {
        let c = format!("alpha.{kind}");
        let put = format!(r#"{{"objects":[{{"collection":"{c}","key":"k","value":"{{}}","permission_read":2,"permission_write":1}}]}}"#);
        assert_eq!(raw("PUT", "/v2/storage", &bearer, &put).0, 403, "write {c}");
        let read = format!(r#"{{"object_ids":[{{"collection":"{c}","key":"k","user_id":"{}"}}]}}"#, p.id);
        assert_eq!(raw("POST", "/v2/storage", &bearer, &read).0, 403, "read {c}");
        let del = format!(r#"{{"object_ids":[{{"collection":"{c}","key":"k"}}]}}"#);
        assert_eq!(raw("PUT", "/v2/storage/delete", &bearer, &del).0, 403, "delete {c}");
        assert_eq!(raw("GET", &format!("/v2/storage/{c}?limit=5"), &bearer, "").0, 403, "list {c}");
    }
    // Ordinary collections of the game still work.
    let ok = r#"{"objects":[{"collection":"alpha.saves","key":"k","value":"{}","permission_read":1,"permission_write":1}]}"#;
    assert_eq!(raw("PUT", "/v2/storage", &bearer, ok).0, 200);
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn a_player_cannot_choose_their_display_name() {
    let p = player("alpha");
    let bearer = format!("Bearer {}", p.token);
    assert_eq!(raw("PUT", "/v2/account", &bearer, r#"{"username":"Ada"}"#).0, 403, "UpdateAccount is closed");
    assert_eq!(account(&p.token).name, p.name, "and the name did not change");
    // A name chosen at account creation is dropped: Nakama generates one.
    let device = fresh_device_id("alpha");
    let body = format!(r#"{{"id":"{device}"}}"#);
    let (status, text) = raw("POST", "/v2/account/authenticate/device?create=true&username=Ada", &basic(), &body);
    assert_eq!(status, 200, "{text}");
    let marker = "\"token\":\"";
    let start = text.find(marker).unwrap() + marker.len();
    let token = &text[start..start + text[start..].find('"').unwrap()];
    assert_ne!(account(token).name, "Ada", "the sign-in's username must be ignored");
}

fn signed_in(game: &str) -> (ironhold_nakama::NakamaBackend, String) {
    let mut b = adapter(game, &fresh_secret());
    let BackendReply::SignedIn { display_name } = ask(&mut b, 1, RequestKind::SignIn).reply else { panic!("signs in") };
    (b, display_name)
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn the_adapter_runs_a_whole_lobby_against_the_real_server() {
    let ((mut a, a_name), (mut b, b_name)) = (signed_in("alpha"), signed_in("alpha"));
    let create = RequestKind::CreateLobby { name: "Casual".into(), max_players: 4 };
    let BackendReply::LobbyCreated { lobby } = ask(&mut a, 2, create).reply else { panic!("created") };
    assert_eq!((lobby.host.as_str(), lobby.members.clone()), (a_name.as_str(), vec![a_name.clone()]));
    let BackendReply::LobbyListed { rows } = ask(&mut b, 2, RequestKind::ListLobbies).reply else { panic!("listed") };
    let row = rows.iter().find(|r| r.id == lobby.id).expect("the new lobby is listed");
    assert_eq!((row.players, row.max, row.host.as_str()), (1, 4, a_name.as_str()));
    let BackendReply::LobbyJoined { lobby: joined } = ask(&mut b, 3, RequestKind::JoinLobby { lobby_id: lobby.id.clone() }).reply else { panic!("joined") };
    assert_eq!(joined.members, vec![a_name.clone(), b_name.clone()]);
    let BackendReply::LobbyState { lobby: seen } = ask(&mut a, 3, RequestKind::LobbyHeartbeat { lobby_id: lobby.id.clone() }).reply else { panic!("state") };
    assert_eq!(seen.members.len(), 2);
    assert_eq!(ask(&mut b, 4, RequestKind::StartLobby { lobby_id: lobby.id.clone() }).reply, BackendReply::LobbyStartFailed { reason: "not_host".into() });
    let BackendReply::LobbyStarted { lobby: started } = ask(&mut a, 4, RequestKind::StartLobby { lobby_id: lobby.id.clone() }).reply else { panic!("started") };
    assert!(started.started && started.version > seen.version);
    let BackendReply::LobbyState { lobby: told } = ask(&mut b, 5, RequestKind::LobbyHeartbeat { lobby_id: lobby.id.clone() }).reply else { panic!("state") };
    assert!(told.started, "a member learns of the start from its next heartbeat");
    assert_eq!(ask(&mut a, 5, RequestKind::LeaveLobby).reply, BackendReply::LobbyLeft);
    // A heartbeat for a lobby the player is not in answers closed, not trouble.
    let reply = ask(&mut a, 6, RequestKind::LobbyHeartbeat { lobby_id: lobby.id.clone() }).reply;
    assert_eq!(reply, BackendReply::LobbyClosed { reason: "removed".into() });
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn signing_out_right_after_a_create_leaves_no_ghost_lobby_on_the_real_server() {
    let ((mut a, a_name), (mut observer, _)) = (signed_in("alpha"), signed_in("alpha"));
    // Core's order for a sign-out while the create is still unanswered: the create, then the leave, then the sign-out.
    a.submit(BackendRequest { id: 2, kind: RequestKind::CreateLobby { name: "Casual".into(), max_players: 4 } });
    a.submit(BackendRequest { id: 3, kind: RequestKind::LeaveLobby });
    a.submit(BackendRequest { id: 4, kind: RequestKind::SignOut });
    let until = std::time::Instant::now() + Duration::from_secs(10);
    let mut out = Vec::new();
    while std::time::Instant::now() < until && !(out.iter().any(|e: &ironhold_core::runtime::backend::BackendEvent| e.request_id == 4)) {
        a.poll(&mut out);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(out.iter().any(|e| e.request_id == 4 && e.reply == BackendReply::SignedOut), "{out:?}");
    // The teardown (create, leave, logout) finishes in the background: keep polling, then look at the server.
    let until = std::time::Instant::now() + Duration::from_secs(5);
    let mut ghost = true;
    while std::time::Instant::now() < until && ghost {
        a.poll(&mut out);
        let BackendReply::LobbyListed { rows } = ask(&mut observer, 9, RequestKind::ListLobbies).reply else { panic!("listed") };
        ghost = rows.iter().any(|r| r.host == a_name);
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!ghost, "the signed-out player's lobby is still listed");
}

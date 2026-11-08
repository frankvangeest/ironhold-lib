//! Contract test of the adapter and the generic Lua module against a real Nakama. Run with `tests/contract/run.sh` (a throwaway
//! local container); ignored otherwise. Games `alpha` and `beta` come from `tests/contract/modules/ironhold_games.lua`.

use std::time::{Duration, Instant};

use ironhold_core::runtime::backend::{Backend, BackendEvent, BackendReply, BackendRequest, RequestKind, ResolvedBackendConfig};
use ironhold_core::schema::backend::BackendConfig;
use ironhold_nakama::{NakamaBackend, NakamaOptions};

fn url() -> String {
    std::env::var("NAKAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:7350".to_string())
}

fn key() -> String {
    std::env::var("NAKAMA_SERVER_KEY").unwrap_or_else(|_| "defaultkey".to_string())
}

fn resolved(game: &str) -> ResolvedBackendConfig {
    let c: BackendConfig = ron::from_str(
        r#"BackendConfig(schema_version: 1, endpoint: Some("http://x"),
            leaderboards: [Leaderboard(name: "best", ranking: HigherIsBetter, keep: Best),
                           Leaderboard(name: "fast", ranking: LowerIsBetter, keep: Latest)])"#,
    )
    .unwrap();
    ResolvedBackendConfig { endpoint: url(), game_id: game.to_string(), server_key: None, leaderboards: c.leaderboards }
}

fn adapter(game: &str, secret: &str) -> NakamaBackend {
    NakamaBackend::new(&resolved(game), &NakamaOptions { server_key: key(), device_secret: secret.to_string() }).unwrap()
}

fn ask(b: &mut NakamaBackend, id: u64, kind: RequestKind) -> BackendEvent {
    b.submit(BackendRequest { id, kind });
    let until = Instant::now() + Duration::from_secs(15);
    let mut out = Vec::new();
    while Instant::now() < until {
        b.poll(&mut out);
        if let Some(e) = out.iter().find(|e| e.request_id == id) {
            return e.clone();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no reply for request {id}");
}

/// A device secret in the format the adapter requires (32 lowercase hex characters), unique per call.
fn fresh_secret() -> String {
    // A counter next to the clock: parallel tests can land in the same clock tick.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64;
    format!("{nanos:016x}{:016x}", COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

/// A wire id in the format the server now requires: `{game}:` plus 32 lowercase hex characters, unique per call.
fn fresh_device_id(game: &str) -> String {
    format!("{game}:{}", fresh_secret())
}

fn unique(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    format!("{prefix}-{nanos}")
}

// Raw HTTP, for the requests an honest adapter would never send.
fn raw(method: &str, path: &str, auth: &str, body: &str) -> (u16, String) {
    let mut req = ehttp::Request::post(format!("{}{path}", url()), body.as_bytes().to_vec());
    req.method = ehttp::Method::parse(method).expect("a valid HTTP method");
    req.headers.insert("Content-Type", "application/json");
    req.headers.insert("Authorization", auth.to_string());
    let resp = ehttp::fetch_blocking(&req).expect("the server answered");
    (resp.status, resp.text().unwrap_or("").to_string())
}

fn basic() -> String {
    // base64("{key}:") without a dependency in the test.
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let input = format!("{}:", key());
    let mut out = String::new();
    for chunk in input.as_bytes().chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |a, (i, b)| a | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    format!("Basic {out}")
}

/// Status and body of a raw device sign-in (`create=true`), for the refusals whose message matters.
fn raw_signin(device_id: &str) -> (u16, String) {
    raw("POST", "/v2/account/authenticate/device?create=true", &basic(), &format!(r#"{{"id":"{device_id}"}}"#))
}

/// `(token, refresh_token)` of a raw device sign-in, with whatever extra body the test wants to try.
fn raw_session(device_id: &str, extra_vars: &str) -> Option<(String, String)> {
    let body = format!(r#"{{"id":"{device_id}"{extra_vars}}}"#);
    let (status, text) = raw("POST", "/v2/account/authenticate/device?create=true", &basic(), &body);
    if status != 200 {
        return None;
    }
    let field = |name: &str| {
        let marker = format!("\"{name}\":\"");
        let start = text.find(&marker)? + marker.len();
        Some(text[start..start + text[start..].find('"')?].to_string())
    };
    Some((field("token")?, field("refresh_token")?))
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn the_adapter_signs_in_submits_and_lists_with_keep_best() {
    let secret = fresh_secret();
    let mut b = adapter("alpha", &secret);
    assert!(matches!(ask(&mut b, 1, RequestKind::SignIn).reply, BackendReply::SignedIn { .. }));
    let submit = |score| RequestKind::SubmitScore { leaderboard: "best".into(), score };
    assert!(matches!(ask(&mut b, 2, submit(50)).reply, BackendReply::ScoreSubmitted { .. }));
    assert!(matches!(ask(&mut b, 3, submit(30)).reply, BackendReply::ScoreSubmitted { .. }));
    match ask(&mut b, 4, RequestKind::ListLeaderboard { leaderboard: "best".into(), count: 10 }).reply {
        BackendReply::Leaderboard { rows, my_rank, my_best, .. } => {
            assert!(rows.iter().any(|r| r.score == 50), "{rows:?}");
            assert_eq!(my_best, Some(50), "keep: Best must hold the better score");
            assert!(my_rank.is_some());
        }
        other => panic!("{other:?}"),
    }
    // Lower-is-better, keep Latest: the last submitted value stands.
    let fast = |score| RequestKind::SubmitScore { leaderboard: "fast".into(), score };
    ask(&mut b, 5, fast(90));
    ask(&mut b, 6, fast(120));
    match ask(&mut b, 7, RequestKind::ListLeaderboard { leaderboard: "fast".into(), count: 10 }).reply {
        BackendReply::Leaderboard { my_best, .. } => assert_eq!(my_best, Some(120)),
        other => panic!("{other:?}"),
    }
    // A leaderboard the server does not know (not declared in ironhold_games.lua) is a per-operation failure.
    let unknown = RequestKind::SubmitScore { leaderboard: "nope".into(), score: 1 };
    assert!(matches!(ask(&mut b, 8, unknown).reply, BackendReply::ScoreFailed { .. }));
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn a_client_for_game_a_cannot_touch_game_b() {
    let (token, refresh) = raw_session(&fresh_device_id("alpha"), "").expect("an alpha device signs in");
    let bearer = format!("Bearer {token}");
    let write = |auth: &str, board: &str| raw("POST", &format!("/v2/leaderboard/{board}"), auth, r#"{"score":"5"}"#).0;
    assert_eq!(write(&bearer, "alpha.best"), 200, "own leaderboard works");
    assert_eq!(write(&bearer, "beta.best"), 403, "another game's leaderboard is refused");
    assert_eq!(raw("GET", "/v2/leaderboard/beta.best?limit=5", &bearer, "").0, 403);
    assert_eq!(raw("GET", "/v2/leaderboard/beta.best?limit=5&owner_ids=x", &bearer, "").0, 403);
    assert_eq!(raw("GET", "/v2/leaderboard/beta.best/owner/x?limit=1", &bearer, "").0, 403, "around-owner is guarded too");
    // Storage collections follow the same rule.
    let put = |c: &str| {
        let body = format!(r#"{{"objects":[{{"collection":"{c}","key":"k","value":"{{}}","permission_read":1,"permission_write":1}}]}}"#);
        raw("PUT", "/v2/storage", &bearer, &body).0
    };
    assert_eq!(put("alpha.saves"), 200);
    assert_eq!(put("beta.saves"), 403);
    assert_eq!(put("saves"), 403, "an unprefixed collection belongs to no game");
    // A client may not pick its own operator per write: a lower score with operator "set" must not replace a better one.
    let write_op = |score: &str, op: &str| {
        raw("POST", "/v2/leaderboard/alpha.best", &bearer, &format!(r#"{{"score":"{score}","operator":"{op}"}}"#)).0
    };
    assert_eq!(write_op("500", "best"), 200);
    write_op("1", "set");
    let (_, listing) = raw("GET", "/v2/leaderboard/alpha.best?limit=100", &bearer, "");
    assert!(listing.contains(r#""score":"500""#), "the client's operator override must be ignored: {listing}");
    // A board the config does not declare is unreachable, even with the right prefix.
    assert_eq!(write(&bearer, "alpha.undeclared"), 403);
    // APIs the engine does not use are closed: account linking would put one account in two games.
    let (status, _) = raw("POST", "/v2/account/link/device", &bearer, r#"{"id":"beta:0123456789abcdef"}"#);
    assert_eq!(status, 403, "LinkDevice is closed");
    assert_eq!(raw("GET", "/v2/match?limit=5", &bearer, "").0, 403, "match listing is closed");
    // The client cannot choose its game through session vars at sign-in...
    let (spoof, _) = raw_session(&fresh_device_id("alpha"), r#","vars":{"game_id":"beta"}"#).expect("signs in");
    assert_eq!(write(&format!("Bearer {spoof}"), "beta.best"), 403, "vars from the client must be ignored");
    // ...nor through a refresh, which must also keep the game (fail closed if it did not).
    let (status, text) = raw("POST", "/v2/account/session/refresh", &basic(), &format!(r#"{{"token":"{refresh}","vars":{{"game_id":"beta"}}}}"#));
    assert_eq!(status, 200, "{text}");
    let new_token = {
        let start = text.find("\"token\":\"").unwrap() + 9;
        text[start..start + text[start..].find('"').unwrap()].to_string()
    };
    let refreshed = format!("Bearer {new_token}");
    assert_eq!(write(&refreshed, "beta.best"), 403, "a refresh must not change the game");
    assert_eq!(write(&refreshed, "alpha.best"), 200, "a refreshed session still works for its own game");
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn unknown_games_and_other_sign_in_methods_are_refused() {
    assert!(raw_session(&fresh_device_id("delta"), "").is_none(), "a game the server does not list");
    assert!(raw_session(&unique("no-prefix-device-id"), "").is_none(), "a device id without a game prefix");
    let (status, _) = raw("POST", "/v2/account/authenticate/email?create=true", &basic(), r#"{"email":"a@b.example","password":"password123"}"#);
    assert_ne!(status, 200, "email sign-in would give a session with no game");
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn a_device_id_must_be_a_game_and_a_32_hex_hash() {
    // The format the adapter sends works.
    assert!(raw_session(&fresh_device_id("alpha"), "").is_some());
    // The old format `{game}:{uuid}` is refused with its own message, so a stale tab or an old build knows what to do.
    for legacy in ["alpha:0123456789abcdef0123", "alpha:6f1c2b9e-0000-4a11-9a00-ironholdv2a", "alpha:00112233445566778899AABBCCDDEEFF", "alpha:00112233445566778899aabbccddeeff0", "alpha:"] {
        let (status, text) = raw_signin(legacy);
        assert_ne!(status, 200, "{legacy}");
        assert!(text.contains("out of date"), "{legacy}: {text}");
    }
    // The game is checked first, so an unknown game still says so, and the format never decides which game is unknown.
    let (status, text) = raw_signin("delta:0123456789abcdef0123");
    assert_ne!(status, 200);
    assert!(text.contains("unknown game"), "{text}");
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn one_secret_gives_two_unrelated_accounts_in_two_games() {
    let secret = fresh_secret();
    let (mut a, mut b) = (adapter("alpha", &secret), adapter("beta", &secret));
    let BackendReply::SignedIn { display_name: name_a } = ask(&mut a, 1, RequestKind::SignIn).reply else { panic!("alpha signs in") };
    let BackendReply::SignedIn { display_name: name_b } = ask(&mut b, 1, RequestKind::SignIn).reply else { panic!("beta signs in") };
    // The server made two accounts (each gets its own random name), and the ids differ beyond the prefix, so it cannot link them.
    assert_ne!(name_a, name_b, "two games, one secret: two accounts");
    let id = |game: &str| ironhold_nakama::ns::GameNs::new(game).unwrap().device_id(&secret).unwrap();
    assert_ne!(id("alpha").split(':').nth(1), id("beta").split(':').nth(1));
}

/// Calls `POST /v2/rpc/{id}` with a session token (`bearer`) or with the HTTP key alone (`None`, no session).
fn rpc(id: &str, bearer: Option<&str>) -> (u16, String) {
    match bearer {
        Some(t) => raw("POST", &format!("/v2/rpc/{id}"), &format!("Bearer {t}"), r#""{}""#),
        None => raw("POST", &format!("/v2/rpc/{id}?http_key=defaulthttpkey"), "", r#""{}""#),
    }
}

#[test]
#[ignore = "needs the local contract Nakama (tests/contract/run.sh)"]
fn lobby_rpcs_go_through_the_game_guard() {
    let (alpha, _) = raw_session(&fresh_device_id("alpha"), "").expect("alpha signs in");
    let (beta, _) = raw_session(&fresh_device_id("beta"), "").expect("beta signs in");

    // A guarded RPC gets the game of the session, never one the client names.
    let (status, body) = rpc("ironhold_lobby_probe", Some(&alpha));
    assert!(status == 200 && body.contains(r#"\"game_id\":\"alpha\""#), "alpha session: {status} {body}");
    let (status, body) = rpc("ironhold_lobby_probe", Some(&beta));
    assert!(status == 200 && body.contains(r#"\"game_id\":\"beta\""#), "beta session: {status} {body}");

    // No session (HTTP key only): the wrapper refuses before the function runs.
    let (status, body) = rpc("ironhold_lobby_probe", None);
    assert!(status == 403 && body.contains("needs a signed-in game session"), "the guard's own refusal, not a 401 or 500: {status} {body}");

    // A refreshed session that asks for another game's vars still belongs to its own game.
    let (_, refresh) = raw_session(&fresh_device_id("alpha"), "").expect("alpha signs in");
    let (status, text) = raw("POST", "/v2/account/session/refresh", &basic(), &format!(r#"{{"token":"{refresh}","vars":{{"game_id":"beta"}}}}"#));
    assert_eq!(status, 200, "{text}");
    let marker = "\"token\":\"";
    let start = text.find(marker).expect("a token") + marker.len();
    let refreshed = &text[start..start + text[start..].find('"').unwrap()];
    let (status, body) = rpc("ironhold_lobby_probe", Some(refreshed));
    assert!(status == 200 && body.contains(r#"\"game_id\":\"alpha\""#), "refreshed session: {status} {body}");

    // A function that errors answers with a fixed message: no module path, no Lua error text.
    let (status, body) = rpc("ironhold_lobby_boom", Some(&alpha));
    assert!(status == 403 && body.contains("the request failed"), "{status} {body}");
    assert!(!body.contains("secret internal detail") && !body.contains(".lua"), "internal detail leaked: {body}");

    // Registration outside the allowlist is refused by `game_rpc` itself (reported by the fixture: one `true` per bad id).
    let (status, body) = rpc("test_registration_report", Some(&alpha));
    assert_eq!(status, 200, "{body}");
    assert!(body.matches("true").count() == 6 && !body.contains("false"), "bad ids must all be refused at registration: {body}");
}

#[path = "contract/lobbies.rs"]
mod lobbies;

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
    ResolvedBackendConfig { endpoint: url(), game_id: game.to_string(), leaderboards: c.leaderboards }
}

fn adapter(game: &str, uuid: &str) -> NakamaBackend {
    NakamaBackend::new(&resolved(game), &NakamaOptions { server_key: key(), device_uuid: uuid.to_string() }).unwrap()
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
    let uuid = unique("contract-a");
    let mut b = adapter("alpha", &uuid);
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
    let (token, refresh) = raw_session(&format!("alpha:{}", unique("adv")), "").expect("an alpha device signs in");
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
    let (spoof, _) = raw_session(&format!("alpha:{}", unique("spoof")), r#","vars":{"game_id":"beta"}"#).expect("signs in");
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
    assert!(raw_session(&format!("gamma:{}", unique("x")), "").is_none(), "a game the server does not list");
    assert!(raw_session(&unique("no-prefix-device-id"), "").is_none(), "a device id without a game prefix");
    let (status, _) = raw("POST", "/v2/account/authenticate/email?create=true", &basic(), r#"{"email":"a@b.example","password":"password123"}"#);
    assert_ne!(status, 200, "email sign-in would give a session with no game");
}

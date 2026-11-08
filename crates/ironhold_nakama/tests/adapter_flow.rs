//! The adapter against an in-process stub HTTP server (no Nakama needed): sign-in, submit, list, a 401 that refreshes and
//! retries, and an unreachable server. The live check against the LAN Nakama is `tests/live_lan.rs` (ignored by default).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ironhold_core::runtime::backend::{
    Backend, BackendEvent, BackendReply, BackendRequest, RequestKind, ResolvedBackendConfig,
};
use ironhold_core::schema::backend::BackendConfig;
use ironhold_nakama::{NakamaBackend, NakamaOptions};

/// `(method, path, authorization)` of every request the stub saw.
type Seen = Arc<Mutex<Vec<(String, String, String)>>>;

fn jwt(uid: &str, usn: &str) -> String {
    // base64url of {"uid":..,"usn":..} without a dependency: the payload is fixed text per call site.
    fn b64(input: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in input.chunks(3) {
            let n = chunk.iter().enumerate().fold(0u32, |a, (i, b)| a | (u32::from(*b) << (16 - 8 * i)));
            for i in 0..=chunk.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            }
        }
        out
    }
    format!("h.{}.s", b64(format!(r#"{{"uid":"{uid}","usn":"{usn}"}}"#).as_bytes()))
}

fn serve(handler: impl Fn(&str, &str, &str, &str) -> (u16, String) + Send + Sync + 'static) -> (String, Seen, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let stop = Arc::new(AtomicBool::new(false));
    let (seen2, stop2) = (seen.clone(), stop.clone());
    let handler = Arc::new(handler);
    std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        while !stop2.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let (h, s) = (handler.clone(), seen2.clone());
                    std::thread::spawn(move || answer(stream, &*h, &s));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(5)),
            }
        }
    });
    (url, seen, stop)
}

fn answer(mut stream: TcpStream, handler: &dyn Fn(&str, &str, &str, &str) -> (u16, String), seen: &Seen) {
    stream.set_nonblocking(false).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    let head_end = loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let length: usize = head
        .lines()
        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0)))
        .unwrap_or(0);
    while buf.len() < head_end + length {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&buf[head_end..]).to_string();
    let mut first = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
    let auth = head
        .lines()
        .find_map(|l| l.to_ascii_lowercase().starts_with("authorization:").then(|| l[14..].trim().to_string()))
        .unwrap_or_default();
    seen.lock().unwrap().push((method.clone(), path.clone(), auth.clone()));
    let (status, payload) = handler(&method, &path, &auth, &body);
    let _ = write!(
        stream,
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
}

fn config(endpoint: &str) -> ResolvedBackendConfig {
    let parsed: BackendConfig = ron::from_str(
        r#"BackendConfig(schema_version: 1, endpoint: Some("http://x"),
            leaderboards: [Leaderboard(name: "best", ranking: HigherIsBetter, keep: Best)])"#,
    )
    .unwrap();
    ResolvedBackendConfig { endpoint: endpoint.to_string(), game_id: "my_game".to_string(), server_key: None, leaderboards: parsed.leaderboards }
}

fn adapter(endpoint: &str) -> NakamaBackend {
    NakamaBackend::new(&config(endpoint), &NakamaOptions { server_key: "k".into(), device_secret: "0123456789abcdef0123456789abcdef".into() }).unwrap()
}

fn ask(b: &mut NakamaBackend, id: u64, kind: RequestKind) -> BackendEvent {
    b.submit(BackendRequest { id, kind });
    let until = Instant::now() + Duration::from_secs(10);
    let mut out = Vec::new();
    while Instant::now() < until {
        b.poll(&mut out);
        if let Some(e) = out.iter().find(|e| e.request_id == id) {
            return e.clone();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("no reply for request {id}");
}

const SECRET: &str = "0123456789abcdef0123456789abcdef";

/// Records every device sign-in body that does not carry the per-game hash, or that carries the secret itself. The handler runs on
/// the server thread, so a violation is collected and asserted by the test.
fn check_wire_id(body: &str, violations: &Mutex<Vec<String>>) {
    let expected = ironhold_nakama::ns::GameNs::new("my_game").unwrap().device_id(SECRET).unwrap();
    if !body.contains(&format!("\"{expected}\"")) || body.contains(SECRET) {
        violations.lock().unwrap().push(body.to_string());
    }
}

fn submit_kind(score: i64) -> RequestKind {
    RequestKind::SubmitScore { leaderboard: "best".into(), score }
}

#[test]
fn sign_in_submit_and_list_use_the_namespaced_ids() {
    let (url, seen, stop) = serve(|method, path, _auth, _body| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => {
            (200, format!(r#"{{"token":"{}","refresh_token":"r1"}}"#, jwt("u1", "Brave Otter")))
        }
        ("POST", "/v2/leaderboard/my_game.best") => (200, "{}".to_string()),
        ("GET", p) if p.starts_with("/v2/leaderboard/my_game.best?") => (
            200,
            r#"{"records":[{"owner_id":"x","username":"Ada","score":"900","rank":"1"}],"owner_records":[{"owner_id":"u1","score":"40"}]}"#.to_string(),
        ),
        // The list carried no rank for the player, so the adapter asks around the owner.
        ("GET", p) if p.starts_with("/v2/leaderboard/my_game.best/owner/u1") => {
            (200, r#"{"records":[{"owner_id":"u1","score":"40","rank":"7"}]}"#.to_string())
        }
        _ => (404, r#"{"message":"nope"}"#.to_string()),
    });
    let mut b = adapter(&url);
    assert_eq!(ask(&mut b, 1, RequestKind::SignIn).reply, BackendReply::SignedIn { display_name: "Brave Otter".into() });
    assert_eq!(ask(&mut b, 2, submit_kind(40)).reply, BackendReply::ScoreSubmitted { leaderboard: "best".into() });
    match ask(&mut b, 3, RequestKind::ListLeaderboard { leaderboard: "best".into(), count: 5 }).reply {
        BackendReply::Leaderboard { rows, my_rank, my_best, .. } => {
            assert_eq!((rows.len(), rows[0].name.as_str(), rows[0].score), (1, "Ada", 900));
            assert_eq!((my_rank, my_best), (Some(7), Some(40)));
        }
        other => panic!("{other:?}"),
    }
    let log = seen.lock().unwrap().clone();
    assert!(log[0].1.contains("create=true") && log[0].2.starts_with("Basic "), "{log:?}");
    assert!(log[1].2.starts_with("Bearer "), "{log:?}");
    assert!(log[2].1.contains("owner_ids=u1") && log[2].1.contains("limit=5"), "{log:?}");
    assert!(log[3].1.contains("/owner/u1"), "{log:?}");
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn an_expired_token_refreshes_once_and_retries() {
    let submits = Arc::new(AtomicUsize::new(0));
    let counter = submits.clone();
    let (url, seen, stop) = serve(move |method, path, _auth, _body| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => {
            (200, format!(r#"{{"token":"{}","refresh_token":"r1"}}"#, jwt("u1", "A")))
        }
        ("POST", "/v2/account/session/refresh") => (200, format!(r#"{{"token":"{}","refresh_token":"r2"}}"#, jwt("u1", "A"))),
        ("POST", "/v2/leaderboard/my_game.best") => {
            counter.fetch_add(1, Ordering::Relaxed);
            // The first call finds the token stale; the retry after the refresh is accepted.
            if counter.load(Ordering::Relaxed) == 1 {
                (401, r#"{"message":"Auth token invalid"}"#.to_string())
            } else {
                (200, "{}".to_string())
            }
        }
        _ => (404, "{}".to_string()),
    });
    let mut b = adapter(&url);
    ask(&mut b, 1, RequestKind::SignIn);
    assert_eq!(ask(&mut b, 2, submit_kind(1)).reply, BackendReply::ScoreSubmitted { leaderboard: "best".into() });
    assert_eq!(submits.load(Ordering::Relaxed), 2, "{:?}", seen.lock().unwrap());
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn a_rejected_sign_in_and_a_missing_leaderboard_carry_short_reasons() {
    let (url, _seen, stop) = serve(|method, path, _a, _b| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => (401, r#"{"message":"Server key invalid"}"#.to_string()),
        _ => (404, r#"{"message":"Leaderboard not found."}"#.to_string()),
    });
    let mut b = adapter(&url);
    assert!(matches!(ask(&mut b, 1, RequestKind::SignIn).reply, BackendReply::SignInFailed { .. }));
    // Not signed in: refused locally, with the per-operation failure.
    match ask(&mut b, 2, submit_kind(1)).reply {
        BackendReply::ScoreFailed { reason, .. } => assert_eq!(reason, "not signed in"),
        other => panic!("{other:?}"),
    }
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn sign_out_drops_the_session_and_an_unreachable_server_says_so() {
    let (url, _seen, stop) = serve(|method, path, _a, _b| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => {
            (200, format!(r#"{{"token":"{}","refresh_token":"r"}}"#, jwt("u1", "A")))
        }
        _ => (200, "{}".to_string()),
    });
    let mut b = adapter(&url);
    ask(&mut b, 1, RequestKind::SignIn);
    assert_eq!(ask(&mut b, 2, RequestKind::SignOut).reply, BackendReply::SignedOut);
    assert!(matches!(ask(&mut b, 3, submit_kind(1)).reply, BackendReply::ScoreFailed { .. }));
    stop.store(true, Ordering::Relaxed);
    // Nothing listens on port 1.
    let mut dead = adapter("http://127.0.0.1:1");
    assert!(matches!(ask(&mut dead, 1, RequestKind::SignIn).reply, BackendReply::Unreachable { .. }));
}

#[test]
fn bad_configuration_is_refused_at_startup() {
    let opts = NakamaOptions { server_key: "k".into(), device_secret: "0123456789abcdef0123456789abcdef".into() };
    assert!(NakamaBackend::new(&config("ftp://x"), &opts).is_err());
    let no_key = NakamaOptions { server_key: " ".into(), device_secret: opts.device_secret.clone() };
    assert!(NakamaBackend::new(&config("http://x"), &no_key).is_err());
    let mut bad_game = config("http://x");
    bad_game.game_id = "Bad Game".into();
    assert!(NakamaBackend::new(&bad_game, &opts).is_err());
}

fn poll_for(b: &mut NakamaBackend, ms: u64) -> Vec<BackendEvent> {
    let until = Instant::now() + Duration::from_millis(ms);
    let mut out = Vec::new();
    while Instant::now() < until {
        b.poll(&mut out);
        std::thread::sleep(Duration::from_millis(5));
    }
    out
}

#[test]
fn a_refused_refresh_signs_in_again_with_the_device_id_and_the_call_succeeds() {
    let auths = Arc::new(AtomicUsize::new(0));
    let (a2, calls) = (auths.clone(), Arc::new(AtomicUsize::new(0)));
    let violations: Arc<Mutex<Vec<String>>> = Arc::default();
    let v2 = violations.clone();
    let (url, _seen, stop) = serve(move |method, path, _auth, body| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => {
            check_wire_id(body, &v2);
            a2.fetch_add(1, Ordering::Relaxed);
            (200, format!(r#"{{"token":"{}","refresh_token":"r"}}"#, jwt("u1", "A")))
        }
        // The refresh token has expired.
        ("POST", "/v2/account/session/refresh") => (401, r#"{"message":"Refresh token invalid"}"#.to_string()),
        ("POST", "/v2/leaderboard/my_game.best") => {
            if calls.fetch_add(1, Ordering::Relaxed) == 0 {
                (401, r#"{"message":"Auth token invalid"}"#.to_string())
            } else {
                (200, "{}".to_string())
            }
        }
        _ => (404, "{}".to_string()),
    });
    let mut b = adapter(&url);
    ask(&mut b, 1, RequestKind::SignIn);
    assert_eq!(ask(&mut b, 2, submit_kind(1)).reply, BackendReply::ScoreSubmitted { leaderboard: "best".into() });
    assert_eq!(auths.load(Ordering::Relaxed), 2, "one sign-in, one re-authentication");
    assert!(violations.lock().unwrap().is_empty(), "the sign-in and the re-authentication both send the per-game hash and never the secret: {:?}", violations.lock().unwrap());
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn two_calls_that_both_expire_share_one_renewal() {
    let refreshes = Arc::new(AtomicUsize::new(0));
    let (r2, calls) = (refreshes.clone(), Arc::new(AtomicUsize::new(0)));
    let (url, _seen, stop) = serve(move |method, path, _auth, _body| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => {
            (200, format!(r#"{{"token":"{}","refresh_token":"r"}}"#, jwt("u1", "A")))
        }
        ("POST", "/v2/account/session/refresh") => {
            r2.fetch_add(1, Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(200));
            (200, format!(r#"{{"token":"{}","refresh_token":"r2"}}"#, jwt("u1", "A")))
        }
        ("POST", "/v2/leaderboard/my_game.best") => {
            // The first two calls find the token stale; the retries are accepted.
            if calls.fetch_add(1, Ordering::Relaxed) < 2 {
                (401, "{}".to_string())
            } else {
                (200, "{}".to_string())
            }
        }
        _ => (404, "{}".to_string()),
    });
    let mut b = adapter(&url);
    ask(&mut b, 1, RequestKind::SignIn);
    b.submit(BackendRequest { id: 2, kind: submit_kind(1) });
    b.submit(BackendRequest { id: 3, kind: submit_kind(2) });
    let got = poll_for(&mut b, 1500);
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(got.iter().all(|e| matches!(e.reply, BackendReply::ScoreSubmitted { .. })), "{got:?}");
    assert_eq!(refreshes.load(Ordering::Relaxed), 1);
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn sign_out_answers_what_is_in_flight_and_a_late_sign_in_changes_nothing() {
    let (url, _seen, stop) = serve(|method, path, _a, _b| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => {
            std::thread::sleep(Duration::from_millis(300));
            (200, format!(r#"{{"token":"{}","refresh_token":"r"}}"#, jwt("u1", "A")))
        }
        _ => (200, "{}".to_string()),
    });
    let mut b = adapter(&url);
    b.submit(BackendRequest { id: 1, kind: RequestKind::SignIn });
    b.submit(BackendRequest { id: 2, kind: RequestKind::SignOut });
    let got = poll_for(&mut b, 800);
    assert_eq!(got.len(), 2, "exactly one reply per request: {got:?}");
    assert!(matches!(&got[0], BackendEvent { request_id: 1, reply: BackendReply::SignInFailed { .. } }), "{got:?}");
    assert_eq!(got[1], BackendEvent { request_id: 2, reply: BackendReply::SignedOut });
    // The late authenticate answer must not have installed a session.
    assert!(matches!(ask(&mut b, 3, submit_kind(1)).reply, BackendReply::ScoreFailed { .. }));
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn a_cancelled_request_makes_no_retry() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c2 = calls.clone();
    let (url, _seen, stop) = serve(move |method, path, _a, _b| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => {
            (200, format!(r#"{{"token":"{}","refresh_token":"r"}}"#, jwt("u1", "A")))
        }
        ("POST", "/v2/account/session/refresh") => {
            std::thread::sleep(Duration::from_millis(200));
            (200, format!(r#"{{"token":"{}","refresh_token":"r2"}}"#, jwt("u1", "A")))
        }
        ("POST", "/v2/leaderboard/my_game.best") => {
            c2.fetch_add(1, Ordering::Relaxed);
            (401, "{}".to_string())
        }
        _ => (404, "{}".to_string()),
    });
    let mut b = adapter(&url);
    ask(&mut b, 1, RequestKind::SignIn);
    b.submit(BackendRequest { id: 2, kind: submit_kind(1) });
    // Wait until the first call was answered (it is now waiting for the renewal), then the engine gives up on it.
    let until = Instant::now() + Duration::from_secs(5);
    while calls.load(Ordering::Relaxed) == 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(50));
    let mut early = Vec::new();
    b.poll(&mut early);
    b.cancel(2);
    let got = poll_for(&mut b, 700);
    assert!(got.iter().all(|e| e.request_id != 2), "no reply after a cancel: {got:?}");
    assert_eq!(calls.load(Ordering::Relaxed), 1, "the call was not retried after the cancel");
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn the_key_goes_with_the_files_endpoint_and_the_device_secret_is_only_made_when_a_server_is_used() {
    use ironhold_nakama::{factory, resolve_server_key, RunnerOptions};
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    assert_eq!(resolve_server_key(Some("file"), Some("runner")).as_deref(), Some("file"));
    assert_eq!(resolve_server_key(None, Some("runner")).as_deref(), Some("runner"), "no key in the file: the runner's fills in");
    assert_eq!(resolve_server_key(Some("  "), Some(" runner ")).as_deref(), Some("runner"), "blank is absent, and values are trimmed");
    assert_eq!(resolve_server_key(None, None), None);

    let made = Arc::new(AtomicUsize::new(0));
    let counter = made.clone();
    let f = factory(RunnerOptions {
        runner_key: None,
        missing_key_hint: "hint".into(),
        device_secret: Arc::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
            Ok("0123456789abcdef0123456789abcdef".to_string())
        }),
    });
    let mut c = config("http://127.0.0.1:1");
    assert_eq!((f.0)(&c).err().as_deref(), Some("hint"), "no key anywhere: the hint is the reason");
    assert_eq!(made.load(Ordering::Relaxed), 0, "no device id is made when no server can be used");
    c.server_key = Some("file".into());
    assert!((f.0)(&c).is_ok());
    assert_eq!(made.load(Ordering::Relaxed), 1);

    let failing = factory(RunnerOptions {
        runner_key: Some("runner".into()),
        missing_key_hint: String::new(),
        device_secret: Arc::new(|| Err("no storage".to_string())),
    });
    assert_eq!((failing.0)(&config("http://127.0.0.1:1")).err().as_deref(), Some("no storage"));

    // A runner key alone is enough when the file names none (a `--backend` run drops the file's key).
    let runner_only = factory(RunnerOptions {
        runner_key: Some("runner".into()),
        missing_key_hint: String::new(),
        device_secret: Arc::new(|| Ok("0123456789abcdef0123456789abcdef".to_string())),
    });
    let c = config("http://127.0.0.1:1");
    assert_eq!(c.server_key, None);
    assert!((runner_only.0)(&c).is_ok());

    // A runner key the adapter cannot send is refused with its own reason, not silently sent.
    for bad in ["a:b", "a b", "k\u{e9}y"] {
        assert_eq!(resolve_server_key(None, Some(bad)), None, "{bad:?}");
    }
    let bad_key = factory(RunnerOptions {
        runner_key: Some("a:b".into()),
        missing_key_hint: "hint".into(),
        device_secret: Arc::new(|| Ok("0123456789abcdef0123456789abcdef".to_string())),
    });
    assert!((bad_key.0)(&c).err().unwrap().contains("not valid"));

    // A config that can never connect does not make a device id.
    let made = Arc::new(AtomicUsize::new(0));
    let counter = made.clone();
    let counting = factory(RunnerOptions {
        runner_key: Some("runner".into()),
        missing_key_hint: String::new(),
        device_secret: Arc::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
            Ok("0123456789abcdef0123456789abcdef".to_string())
        }),
    });
    let mut no_game = config("http://127.0.0.1:1");
    no_game.game_id = String::new();
    assert!((counting.0)(&no_game).is_err());
    assert_eq!(made.load(Ordering::Relaxed), 0, "no device id for a config that can never connect");
}

// ---- lobbies (v3a-2) ---------------------------------------------------------------------------------------------------------

use ironhold_core::runtime::backend::{LobbyInfo, LobbyRow};
use serde_json::{json, Value};

/// A 200 answer of an RPC: Nakama wraps the module's JSON string in `{"payload": "<json>"}`.
fn rpc_reply(inner: Value) -> (u16, String) {
    (200, json!({ "payload": inner.to_string() }).to_string())
}

fn lobby_json(id: &str, version: u64, members: &[&str], started: bool) -> Value {
    json!({ "ok": true, "lobby": { "id": id, "version": version, "name": "Casual", "max": 4, "started": started, "host": members[0], "players": members.len(), "members": members } })
}

fn info(id: &str, version: u64, members: &[&str], started: bool) -> LobbyInfo {
    LobbyInfo {
        id: id.into(), version, name: "Casual".into(), max: 4, started, host: members[0].into(),
        members: members.iter().map(|m| m.to_string()).collect(),
    }
}

/// The JSON object a request sent: the body is a JSON string holding the object.
fn sent(body: &str) -> Value {
    let inner: String = serde_json::from_str(body).expect("the RPC body is a JSON string");
    serde_json::from_str(&inner).expect("holding a JSON object")
}

fn device_auth(usn: &str, refresh: &str) -> (u16, String) {
    (200, format!(r#"{{"token":"{}","refresh_token":"{refresh}"}}"#, jwt("u1", usn)))
}

#[test]
fn lobby_calls_become_rpcs_and_their_answers_become_replies_and_fixed_reasons() {
    let bodies: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
    let seen_bodies = bodies.clone();
    let (url, _seen, stop) = serve(move |method, path, _auth, body| {
        if method == "POST" && path.starts_with("/v2/account/authenticate/device") {
            return device_auth("Ada", "r");
        }
        let Some(rpc) = path.strip_prefix("/v2/rpc/") else { return (404, "{}".to_string()) };
        let request = sent(body);
        seen_bodies.lock().unwrap().push((rpc.to_string(), request.clone()));
        let id = request.get("lobby_id").and_then(Value::as_str).unwrap_or("");
        match (rpc, id) {
            ("ironhold_lobby_list", _) => rpc_reply(json!({ "ok": true, "rows": [{ "id": "L1", "name": "Casual", "players": 1, "max": 4, "host": "Ada" }] })),
            ("ironhold_lobby_create", _) => match request["name"].as_str() {
                Some("Casual") => rpc_reply(lobby_json("L1", 1, &["Ada"], false)),
                Some("Cap") => rpc_reply(json!({ "ok": false, "reason": "limit_reached" })),
                _ => rpc_reply(json!({ "ok": false, "reason": "made_up_reason" })),
            },
            ("ironhold_lobby_join", "ok") => rpc_reply(lobby_json("L1", 2, &["Ada", "Bo"], false)),
            ("ironhold_lobby_join", _) => rpc_reply(json!({ "ok": false, "reason": "full" })),
            ("ironhold_lobby_start", "ok") => rpc_reply(lobby_json("L1", 3, &["Ada", "Bo"], true)),
            ("ironhold_lobby_start", _) => rpc_reply(json!({ "ok": false, "reason": "not_host" })),
            ("ironhold_lobby_heartbeat", "ok") => rpc_reply(lobby_json("L1", 4, &["Ada", "Bo"], false)),
            ("ironhold_lobby_heartbeat", "gone_host") => rpc_reply(json!({ "ok": false, "reason": "host_left" })),
            ("ironhold_lobby_heartbeat", "gone_removed") => rpc_reply(json!({ "ok": false, "reason": "removed" })),
            ("ironhold_lobby_heartbeat", "limited") => (429, "{}".to_string()),
            ("ironhold_lobby_heartbeat", "broken") => (403, r#"{"message":"the request failed"}"#.to_string()),
            ("ironhold_lobby_heartbeat", _) => rpc_reply(json!({ "ok": false, "reason": "not_enabled" })),
            ("ironhold_lobby_leave", _) => (500, "<html>".to_string()),
            _ => (404, "{}".to_string()),
        }
    });
    let mut b = adapter(&url);
    // Not signed in: refused locally with the fixed reason, a leave still leaves.
    assert_eq!(ask(&mut b, 1, RequestKind::CreateLobby { name: "Casual".into(), max_players: 4 }).reply, BackendReply::LobbyCreateFailed { reason: "not_signed_in".into() });
    assert_eq!(ask(&mut b, 2, RequestKind::LeaveLobby).reply, BackendReply::LobbyLeft);
    ask(&mut b, 3, RequestKind::SignIn);
    let mut next = 10;
    let mut call = |kind: RequestKind| {
        next += 1;
        ask(&mut b, next, kind).reply
    };
    let row = LobbyRow { id: "L1".into(), name: "Casual".into(), players: 1, max: 4, host: "Ada".into() };
    assert_eq!(call(RequestKind::ListLobbies), BackendReply::LobbyListed { rows: vec![row] });
    assert_eq!(call(RequestKind::CreateLobby { name: "Casual".into(), max_players: 4 }), BackendReply::LobbyCreated { lobby: info("L1", 1, &["Ada"], false) });
    assert_eq!(call(RequestKind::CreateLobby { name: "Cap".into(), max_players: 4 }), BackendReply::LobbyCreateFailed { reason: "limit_reached".into() });
    assert_eq!(call(RequestKind::CreateLobby { name: "Other".into(), max_players: 4 }), BackendReply::LobbyCreateFailed { reason: "server".into() }, "an unknown reason is `server`");
    assert_eq!(call(RequestKind::JoinLobby { lobby_id: "ok".into() }), BackendReply::LobbyJoined { lobby: info("L1", 2, &["Ada", "Bo"], false) });
    assert_eq!(call(RequestKind::JoinLobby { lobby_id: "x".into() }), BackendReply::LobbyJoinFailed { reason: "full".into() });
    assert_eq!(call(RequestKind::StartLobby { lobby_id: "ok".into() }), BackendReply::LobbyStarted { lobby: info("L1", 3, &["Ada", "Bo"], true) });
    assert_eq!(call(RequestKind::StartLobby { lobby_id: "x".into() }), BackendReply::LobbyStartFailed { reason: "not_host".into() });
    let hb = |id: &str| RequestKind::LobbyHeartbeat { lobby_id: id.into() };
    assert_eq!(call(hb("ok")), BackendReply::LobbyState { lobby: info("L1", 4, &["Ada", "Bo"], false) });
    assert_eq!(call(hb("gone_host")), BackendReply::LobbyClosed { reason: "host_left".into() });
    assert_eq!(call(hb("gone_removed")), BackendReply::LobbyClosed { reason: "removed".into() });
    assert_eq!(call(hb("limited")), BackendReply::LobbyHeartbeatFailed { reason: "rate_limited".into() }, "a 429 is trouble reaching the server, not a closed lobby");
    assert_eq!(call(hb("broken")), BackendReply::LobbyHeartbeatFailed { reason: "server".into() });
    assert_eq!(call(hb("other")), BackendReply::LobbyHeartbeatFailed { reason: "not_enabled".into() });
    assert_eq!(call(RequestKind::LeaveLobby), BackendReply::LobbyLeft, "a leave that cannot be confirmed still leaves");
    stop.store(true, Ordering::Relaxed);
    // The requests carried what core sent, as RPC bodies.
    let sent_bodies = bodies.lock().unwrap().clone();
    assert!(sent_bodies.contains(&("ironhold_lobby_create".into(), json!({ "name": "Casual", "max_players": 4 }))), "{sent_bodies:?}");
    assert!(sent_bodies.contains(&("ironhold_lobby_join".into(), json!({ "lobby_id": "ok" }))), "{sent_bodies:?}");
}

#[test]
fn a_lobby_call_with_an_expired_token_refreshes_once_and_retries() {
    let (creates, refreshes) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let (c2, r2) = (creates.clone(), refreshes.clone());
    let (url, _seen, stop) = serve(move |method, path, _auth, _body| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => device_auth("Ada", "r1"),
        ("POST", "/v2/account/session/refresh") => {
            r2.fetch_add(1, Ordering::Relaxed);
            device_auth("Ada", "r2")
        }
        ("POST", "/v2/rpc/ironhold_lobby_create") => {
            if c2.fetch_add(1, Ordering::Relaxed) == 0 {
                (401, r#"{"message":"Auth token invalid"}"#.to_string())
            } else {
                rpc_reply(lobby_json("L1", 1, &["Ada"], false))
            }
        }
        _ => (404, "{}".to_string()),
    });
    let mut b = adapter(&url);
    ask(&mut b, 1, RequestKind::SignIn);
    let reply = ask(&mut b, 2, RequestKind::CreateLobby { name: "Casual".into(), max_players: 4 }).reply;
    assert_eq!(reply, BackendReply::LobbyCreated { lobby: info("L1", 1, &["Ada"], false) });
    assert_eq!((creates.load(Ordering::Relaxed), refreshes.load(Ordering::Relaxed)), (2, 1));
    stop.store(true, Ordering::Relaxed);
}

/// What the stub saw, by RPC name or path, in arrival order.
fn paths(seen: &Seen) -> Vec<String> {
    seen.lock().unwrap().iter().map(|(_, p, _)| p.split('?').next().unwrap().to_string()).collect()
}

fn position(log: &[String], suffix: &str) -> Option<usize> {
    log.iter().position(|p| p.ends_with(suffix))
}

/// Polls the adapter (it only moves when polled) until the server has seen `suffix`; returns the replies that arrived meanwhile.
fn wait_for_path(b: &mut NakamaBackend, seen: &Seen, suffix: &str) -> Vec<BackendEvent> {
    let until = Instant::now() + Duration::from_secs(8);
    let mut out = Vec::new();
    while Instant::now() < until {
        b.poll(&mut out);
        if position(&paths(seen), suffix).is_some() {
            return out;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the server never saw {suffix}: {:?}", paths(seen));
}

#[test]
fn signing_out_during_a_join_lets_the_join_finish_then_leaves_then_logs_out_with_the_old_session() {
    let join_done = Arc::new(AtomicBool::new(false));
    let early_leave = Arc::new(AtomicBool::new(false));
    let (done, early) = (join_done.clone(), early_leave.clone());
    let (url, seen, stop) = serve(move |method, path, auth, _body| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => device_auth("Old", "r"),
        ("POST", "/v2/rpc/ironhold_lobby_join") => {
            std::thread::sleep(Duration::from_millis(400));
            done.store(true, Ordering::SeqCst);
            rpc_reply(lobby_json("L1", 2, &["Ada", "Bo"], false))
        }
        ("POST", "/v2/rpc/ironhold_lobby_leave") => {
            // The leave may only be sent once the join was answered, and with the session the join used.
            if !done.load(Ordering::SeqCst) || !auth.contains(&jwt("u1", "Old")) {
                early.store(true, Ordering::SeqCst);
            }
            rpc_reply(json!({ "ok": true }))
        }
        _ => (200, "{}".to_string()),
    });
    let mut b = adapter(&url);
    ask(&mut b, 1, RequestKind::SignIn);
    // Core's order: the join is on the wire, the engine signs out: the leave is queued before the sign-out.
    b.submit(BackendRequest { id: 2, kind: RequestKind::JoinLobby { lobby_id: "L1".into() } });
    b.submit(BackendRequest { id: 3, kind: RequestKind::LeaveLobby });
    b.submit(BackendRequest { id: 4, kind: RequestKind::SignOut });
    let mut got = poll_for(&mut b, 1500);
    got.sort_by_key(|e| e.request_id);
    assert_eq!(got.iter().map(|e| e.request_id).collect::<Vec<_>>(), vec![2, 3, 4], "one reply each: {got:?}");
    assert_eq!(got[2].reply, BackendReply::SignedOut);
    assert_eq!(got[1].reply, BackendReply::LobbyLeft);
    assert!(!early_leave.load(Ordering::SeqCst), "the leave went out before the join was answered, or with another session");
    let log = paths(&seen);
    let (join, leave, logout) = (position(&log, "lobby_join"), position(&log, "lobby_leave"), position(&log, "/v2/session/logout"));
    assert!(join < leave && leave < logout && logout.is_some(), "join, then leave, then logout: {log:?}");
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn a_sign_in_during_the_teardown_waits_until_the_logout_is_sent() {
    let sign_ins = Arc::new(AtomicUsize::new(0));
    let s2 = sign_ins.clone();
    let (url, seen, stop) = serve(move |method, path, _auth, _body| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => {
            if s2.fetch_add(1, Ordering::SeqCst) == 0 { device_auth("Old", "r") } else { device_auth("New", "r") }
        }
        ("POST", "/v2/rpc/ironhold_lobby_join") => {
            std::thread::sleep(Duration::from_millis(400));
            rpc_reply(lobby_json("L1", 2, &["Ada", "Bo"], false))
        }
        ("POST", "/v2/rpc/ironhold_lobby_leave") => rpc_reply(json!({ "ok": true })),
        _ => (200, "{}".to_string()),
    });
    let mut b = adapter(&url);
    ask(&mut b, 1, RequestKind::SignIn);
    b.submit(BackendRequest { id: 2, kind: RequestKind::JoinLobby { lobby_id: "L1".into() } });
    b.submit(BackendRequest { id: 3, kind: RequestKind::LeaveLobby });
    b.submit(BackendRequest { id: 4, kind: RequestKind::SignOut });
    b.submit(BackendRequest { id: 5, kind: RequestKind::SignIn });
    // While the join is unanswered the second sign-in has not been sent.
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(sign_ins.load(Ordering::SeqCst), 1, "the new sign-in is held: {:?}", paths(&seen));
    let mut got = poll_for(&mut b, 1500);
    got.sort_by_key(|e| e.request_id);
    let ids: Vec<u64> = got.iter().map(|e| e.request_id).collect();
    assert_eq!(ids, vec![2, 3, 4, 5], "{got:?}");
    assert_eq!(got[3].reply, BackendReply::SignedIn { display_name: "New".into() });
    let log = paths(&seen);
    let second_sign_in = log.iter().enumerate().filter(|(_, p)| p.ends_with("/authenticate/device")).nth(1).map(|(i, _)| i);
    assert!(position(&log, "lobby_leave") < second_sign_in, "the old session's leave comes before the new sign-in: {log:?}");
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn a_create_that_never_answers_is_ended_by_the_engines_cancel_and_the_teardown_goes_on() {
    let (url, seen, stop) = serve(|method, path, _auth, _body| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => device_auth("Ada", "r"),
        ("POST", "/v2/rpc/ironhold_lobby_create") => {
            std::thread::sleep(Duration::from_secs(4));
            rpc_reply(lobby_json("L1", 1, &["Ada"], false))
        }
        ("POST", "/v2/rpc/ironhold_lobby_leave") => rpc_reply(json!({ "ok": true })),
        _ => (200, "{}".to_string()),
    });
    let mut b = adapter(&url);
    ask(&mut b, 1, RequestKind::SignIn);
    b.submit(BackendRequest { id: 2, kind: RequestKind::CreateLobby { name: "Casual".into(), max_players: 4 } });
    b.submit(BackendRequest { id: 3, kind: RequestKind::LeaveLobby });
    b.submit(BackendRequest { id: 4, kind: RequestKind::SignOut });
    b.submit(BackendRequest { id: 5, kind: RequestKind::SignIn });
    let early = poll_for(&mut b, 300);
    assert!(early.iter().any(|e| e.request_id == 4 && e.reply == BackendReply::SignedOut), "SignedOut is answered at once: {early:?}");
    assert!(early.iter().all(|e| e.request_id != 5), "the sign-in is held: {early:?}");
    assert_eq!(position(&paths(&seen), "lobby_leave"), None, "the leave waits for the create");
    // Core's timeout fires for the create (and the leave queued with it): the create is dropped, the leave goes out, then the logout.
    b.cancel(2);
    let mut got = wait_for_path(&mut b, &seen, "lobby_leave");
    got.extend(wait_for_path(&mut b, &seen, "/v2/session/logout"));
    got.extend(poll_for(&mut b, 800));
    assert!(got.iter().any(|e| e.request_id == 5 && matches!(e.reply, BackendReply::SignedIn { .. })), "the held sign-in proceeds: {got:?}");
    assert!(got.iter().all(|e| e.request_id != 2), "no reply after a cancel: {got:?}");
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn the_engine_cancelling_the_held_sign_in_leaves_no_stuck_state() {
    let (url, seen, stop) = serve(|method, path, _auth, _body| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => device_auth("Ada", "r"),
        ("POST", "/v2/rpc/ironhold_lobby_join") => {
            std::thread::sleep(Duration::from_millis(300));
            rpc_reply(lobby_json("L1", 2, &["Ada", "Bo"], false))
        }
        ("POST", "/v2/rpc/ironhold_lobby_leave") => rpc_reply(json!({ "ok": true })),
        _ => (200, "{}".to_string()),
    });
    let mut b = adapter(&url);
    ask(&mut b, 1, RequestKind::SignIn);
    b.submit(BackendRequest { id: 2, kind: RequestKind::JoinLobby { lobby_id: "L1".into() } });
    b.submit(BackendRequest { id: 3, kind: RequestKind::LeaveLobby });
    b.submit(BackendRequest { id: 4, kind: RequestKind::SignOut });
    b.submit(BackendRequest { id: 5, kind: RequestKind::SignIn });
    b.cancel(5);
    let got = poll_for(&mut b, 1200);
    assert!(got.iter().all(|e| e.request_id != 5), "the cancelled sign-in is never answered or sent: {got:?}");
    wait_for_path(&mut b, &seen, "/v2/session/logout");
    // The teardown ended: a fresh sign-in works at once.
    assert!(matches!(ask(&mut b, 6, RequestKind::SignIn).reply, BackendReply::SignedIn { .. }));
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn a_401_during_the_teardown_renews_the_old_session_only_and_the_logout_uses_the_renewed_token() {
    let leaves = Arc::new(AtomicUsize::new(0));
    let l2 = leaves.clone();
    let logout_auth: Arc<Mutex<String>> = Arc::default();
    let la = logout_auth.clone();
    let (url, seen, stop) = serve(move |method, path, auth, _body| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => device_auth("Old", "r1"),
        ("POST", "/v2/account/session/refresh") => device_auth("Renewed", "r2"),
        ("POST", "/v2/rpc/ironhold_lobby_leave") => {
            std::thread::sleep(Duration::from_millis(150));
            if l2.fetch_add(1, Ordering::SeqCst) == 0 { (401, "{}".to_string()) } else { rpc_reply(json!({ "ok": true })) }
        }
        ("POST", "/v2/session/logout") => {
            *la.lock().unwrap() = auth.to_string();
            (200, "{}".to_string())
        }
        _ => (404, "{}".to_string()),
    });
    let mut b = adapter(&url);
    ask(&mut b, 1, RequestKind::SignIn);
    b.submit(BackendRequest { id: 2, kind: RequestKind::LeaveLobby });
    b.submit(BackendRequest { id: 3, kind: RequestKind::SignOut });
    let mut got = wait_for_path(&mut b, &seen, "/v2/session/logout");
    assert_eq!(leaves.load(Ordering::SeqCst), 2, "the leave was retried once with the renewed token");
    assert!(logout_auth.lock().unwrap().contains(&jwt("u1", "Renewed")), "{}", logout_auth.lock().unwrap());
    got.extend(poll_for(&mut b, 300));
    assert!(got.iter().any(|e| e.request_id == 3 && e.reply == BackendReply::SignedOut), "{got:?}");
    // The renewal never became the adapter's own session: the player is signed out.
    assert!(matches!(ask(&mut b, 4, submit_kind(1)).reply, BackendReply::ScoreFailed { .. }));
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn a_leave_waits_for_a_join_parked_behind_a_session_renewal() {
    let joins = Arc::new(AtomicUsize::new(0));
    let j2 = joins.clone();
    let early_leave = Arc::new(AtomicBool::new(false));
    let (e2, joined) = (early_leave.clone(), Arc::new(AtomicBool::new(false)));
    let j3 = joined.clone();
    let (url, seen, stop) = serve(move |method, path, _auth, _body| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => device_auth("Old", "r1"),
        ("POST", "/v2/account/session/refresh") => {
            std::thread::sleep(Duration::from_millis(300));
            device_auth("Renewed", "r2")
        }
        ("POST", "/v2/rpc/ironhold_lobby_join") => {
            if j2.fetch_add(1, Ordering::SeqCst) == 0 {
                (401, "{}".to_string())
            } else {
                j3.store(true, Ordering::SeqCst);
                rpc_reply(lobby_json("L1", 2, &["Ada", "Bo"], false))
            }
        }
        ("POST", "/v2/rpc/ironhold_lobby_leave") => {
            if !j3.load(Ordering::SeqCst) {
                e2.store(true, Ordering::SeqCst);
            }
            rpc_reply(json!({ "ok": true }))
        }
        _ => (200, "{}".to_string()),
    });
    let mut b = adapter(&url);
    ask(&mut b, 1, RequestKind::SignIn);
    b.submit(BackendRequest { id: 2, kind: RequestKind::JoinLobby { lobby_id: "L1".into() } });
    // Wait until the join got its 401 and is parked behind the renewal, then core signs out (leave first).
    let mut got = wait_for_path(&mut b, &seen, "/v2/account/session/refresh");
    b.submit(BackendRequest { id: 3, kind: RequestKind::LeaveLobby });
    b.submit(BackendRequest { id: 4, kind: RequestKind::SignOut });
    got.extend(poll_for(&mut b, 1500));
    got.sort_by_key(|e| e.request_id);
    assert_eq!(got.iter().map(|e| e.request_id).collect::<Vec<_>>(), vec![2, 3, 4], "{got:?}");
    assert!(!early_leave.load(Ordering::SeqCst), "the leave must come after the parked join finished");
    wait_for_path(&mut b, &seen, "/v2/session/logout");
    assert_eq!(joins.load(Ordering::SeqCst), 2, "the parked join was retried, not dropped");
    stop.store(true, Ordering::Relaxed);
}

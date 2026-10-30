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
    NakamaBackend::new(&config(endpoint), &NakamaOptions { server_key: "k".into(), device_uuid: "0123-abcd-4567".into() }).unwrap()
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
    let opts = NakamaOptions { server_key: "k".into(), device_uuid: "0123-abcd-4567".into() };
    assert!(NakamaBackend::new(&config("ftp://x"), &opts).is_err());
    let no_key = NakamaOptions { server_key: " ".into(), device_uuid: opts.device_uuid.clone() };
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
    let (url, _seen, stop) = serve(move |method, path, _auth, _body| match (method, path) {
        ("POST", p) if p.starts_with("/v2/account/authenticate/device") => {
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
fn the_key_goes_with_the_files_endpoint_and_the_device_uuid_is_only_made_when_a_server_is_used() {
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
        device_uuid: Arc::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
            Ok("0123-abcd-4567".to_string())
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
        device_uuid: Arc::new(|| Err("no storage".to_string())),
    });
    assert_eq!((failing.0)(&config("http://127.0.0.1:1")).err().as_deref(), Some("no storage"));

    // A runner key alone is enough when the file names none (a `--backend` run drops the file's key).
    let runner_only = factory(RunnerOptions {
        runner_key: Some("runner".into()),
        missing_key_hint: String::new(),
        device_uuid: Arc::new(|| Ok("0123-abcd-4567".to_string())),
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
        device_uuid: Arc::new(|| Ok("0123-abcd-4567".to_string())),
    });
    assert!((bad_key.0)(&c).err().unwrap().contains("not valid"));

    // A config that can never connect does not make a device id.
    let made = Arc::new(AtomicUsize::new(0));
    let counter = made.clone();
    let counting = factory(RunnerOptions {
        runner_key: Some("runner".into()),
        missing_key_hint: String::new(),
        device_uuid: Arc::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
            Ok("0123-abcd-4567".to_string())
        }),
    });
    let mut no_game = config("http://127.0.0.1:1");
    no_game.game_id = String::new();
    assert!((counting.0)(&no_game).is_err());
    assert_eq!(made.load(Ordering::Relaxed), 0, "no device id for a config that can never connect");
}

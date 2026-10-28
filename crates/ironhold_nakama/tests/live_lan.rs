//! Live check against a real Nakama (the homelab LXC on the LAN). Ignored by default; run with
//! The server must host `backend_demo` (it runs the Ironhold module). `NAKAMA_URL=http://<host>:7350 cargo test -p ironhold_nakama --test live_lan -- --ignored --nocapture`.
//! The server key comes from `NAKAMA_SERVER_KEY` or the file named by `NAKAMA_SERVER_KEY_FILE` (default
//! `~/.nakama_spike_server_key`), never from the repository, and is never printed.

use std::time::{Duration, Instant};

use ironhold_core::runtime::backend::{Backend, BackendEvent, BackendReply, BackendRequest, RequestKind, ResolvedBackendConfig};
use ironhold_nakama::{NakamaBackend, NakamaOptions};

fn server_key() -> String {
    if let Ok(k) = std::env::var("NAKAMA_SERVER_KEY") {
        return k.trim().to_string();
    }
    let path = std::env::var_os("NAKAMA_SERVER_KEY_FILE").map(std::path::PathBuf::from).unwrap_or_else(|| {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default();
        std::path::PathBuf::from(home).join(".nakama_spike_server_key")
    });
    std::fs::read_to_string(path).expect("no server key (set NAKAMA_SERVER_KEY or create the key file)").trim().to_string()
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
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no reply for request {id}");
}

#[test]
#[ignore = "needs a reachable Nakama and its server key"]
fn sign_in_and_out_against_a_real_server() {
    let url = std::env::var("NAKAMA_URL").unwrap_or_else(|_| "http://localhost:7350".to_string());
    let resolved = ResolvedBackendConfig { endpoint: url, game_id: "backend_demo".into(), leaderboards: vec![] };
    let options = NakamaOptions { server_key: server_key(), device_uuid: "6f1c2b9e-0000-4a11-9a00-ironholdv2a".into() };
    let mut b = NakamaBackend::new(&resolved, &options).unwrap();
    let signed_in = ask(&mut b, 1, RequestKind::SignIn).reply;
    println!("sign-in reply: {signed_in:?}");
    assert!(matches!(signed_in, BackendReply::SignedIn { .. }), "{signed_in:?}");
    assert_eq!(ask(&mut b, 2, RequestKind::SignOut).reply, BackendReply::SignedOut);
    // A wrong key must be rejected as a sign-in failure.
    let bad = NakamaOptions { server_key: "definitely-wrong".into(), ..options };
    let mut b = NakamaBackend::new(&resolved, &bad).unwrap();
    let rejected = ask(&mut b, 1, RequestKind::SignIn).reply;
    println!("wrong key reply: {rejected:?}");
    assert!(matches!(rejected, BackendReply::SignInFailed { .. }), "{rejected:?}");
}

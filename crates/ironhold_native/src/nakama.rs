//! Wires the Nakama adapter into the native runner. Everything machine-specific stays out of the project: the device secret is
//! generated once and kept in the user's home folder (only when a project connects to a real server), and a server key from
//! `NAKAMA_SERVER_KEY` or a key file fills in when the project's `backend.ron` names none (or when `--backend` points the
//! project at another server, where the file's key must not follow).

use std::path::PathBuf;

use ironhold_core::runtime::backend::BackendFactory;
use ironhold_nakama::RunnerOptions;

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default())
}

fn server_key() -> Option<String> {
    if let Ok(key) = std::env::var("NAKAMA_SERVER_KEY") {
        return Some(key.trim().to_string()).filter(|k| !k.is_empty());
    }
    let path = std::env::var_os("NAKAMA_SERVER_KEY_FILE").map(PathBuf::from).unwrap_or_else(|| home().join(".nakama_server_key"));
    std::fs::read_to_string(path).ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
}

/// A random secret (32 hex characters), created on first use and then stable for this install. The server never sees it: the
/// adapter sends a per-game hash of it. It comes from the OS random source.
fn device_secret() -> Option<String> {
    let dir = home().join(".ironhold");
    // The file the id lived in before the wire id became a per-game hash. Removed on every call (NotFound is fine), so a crash
    // between creating the new secret and deleting the old id cannot leave the raw old id on disk.
    let _ = std::fs::remove_file(dir.join("device_id"));
    let path = dir.join("device_secret");
    if let Ok(existing) = std::fs::read_to_string(&path) {
        // A hand-edited or damaged file must not take the game offline for good: anything unusable is replaced.
        let existing = existing.trim_start_matches('\u{feff}').trim().to_string();
        if ironhold_nakama::ns::is_valid_device_secret(&existing) {
            return Some(existing);
        }
    }
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    let secret = ironhold_nakama::ns::device_secret_from_bytes(&bytes);
    std::fs::create_dir_all(&dir).ok()?;
    std::fs::write(&path, &secret).ok()?;
    Some(secret)
}

/// Always a factory, so a project with a real endpoint says what is missing (`backend.last_error`) instead of blaming the build.
/// The key and the device secret are resolved when the factory runs (after `backend.ron` has loaded), not here.
pub fn backend_factory() -> Option<BackendFactory> {
    Some(ironhold_nakama::factory(RunnerOptions {
        runner_key: server_key(),
        missing_key_hint: "no Nakama server key: set server_key in backend.ron (not sent to a --backend server) or NAKAMA_SERVER_KEY (or ~/.nakama_server_key)".to_string(),
        device_secret: std::sync::Arc::new(|| device_secret().ok_or_else(|| "the device id could not be saved: is your home folder writable?".to_string())),
    }))
}

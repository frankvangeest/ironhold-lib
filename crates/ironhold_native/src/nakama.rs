//! Wires the Nakama adapter into the native runner. Everything machine-specific stays out of the project: the server key
//! comes from `NAKAMA_SERVER_KEY` or a key file, the device uuid is generated once and kept in the user's home folder.

use std::path::PathBuf;

use ironhold_core::runtime::backend::BackendFactory;
use ironhold_nakama::NakamaOptions;

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

/// A random uuid-shaped id, created on first use and then stable for this install. It is a credential for the player's
/// account on the server, so it comes from the OS random source.
fn device_uuid() -> Option<String> {
    let path = home().join(".ironhold").join("device_id");
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let existing = existing.trim().to_string();
        if existing.len() >= 16 {
            return Some(existing);
        }
    }
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let id = format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]);
    std::fs::create_dir_all(path.parent()?).ok()?;
    std::fs::write(&path, &id).ok()?;
    Some(id)
}

/// `None` when this machine has no server key: a project with a real endpoint then runs offline and says so.
pub fn backend_factory() -> Option<BackendFactory> {
    let server_key = server_key()?;
    let device_uuid = device_uuid()?;
    Some(ironhold_nakama::factory(NakamaOptions { server_key, device_uuid }))
}

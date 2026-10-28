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
        // A hand-edited or damaged file must not take the game offline for good: anything unusable is replaced.
        let existing = existing.trim_start_matches('\u{feff}').trim().to_string();
        if (16..=100).contains(&existing.len()) && existing.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
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

/// Always a factory, so a project with a real endpoint says what is missing (`backend.last_error`) instead of blaming the build.
pub fn backend_factory() -> Option<BackendFactory> {
    let missing = |why: &'static str| Some(BackendFactory::new(move |_| Err(why.to_string())));
    let Some(server_key) = server_key() else {
        return missing("no Nakama server key: set NAKAMA_SERVER_KEY (or put it in ~/.nakama_server_key)");
    };
    let Some(device_uuid) = device_uuid() else {
        return missing("the device id could not be saved: is your home folder writable?");
    };
    Some(ironhold_nakama::factory(NakamaOptions { server_key, device_uuid }))
}

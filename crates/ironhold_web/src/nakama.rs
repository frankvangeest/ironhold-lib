//! Wires the Nakama adapter into the web runner. The server key is a client identifier, not a secret (it ships in any web
//! build): it normally comes from the project's `backend.ron` (`server_key`), and `?nakama_key=` in the page URL only fills in
//! when the file names none. The device uuid is generated once per browser profile and kept in `localStorage`, and only when a
//! project actually connects to a real server (a project that runs offline never writes it). Without a key anywhere a project
//! with a real endpoint runs offline and says so.

use ironhold_core::runtime::backend::BackendFactory;
use ironhold_nakama::RunnerOptions;

const STORAGE_KEY: &str = "ironhold.device_id";

/// A random uuid-shaped id from the browser's crypto source, stable per browser profile. It is a credential for the player's
/// account on the server.
fn device_uuid(window: &web_sys::Window) -> Option<String> {
    let storage = window.local_storage().ok().flatten()?;
    if let Ok(Some(existing)) = storage.get_item(STORAGE_KEY) {
        // Anything unusable is replaced, so a damaged value cannot take the game offline for good.
        if (16..=100).contains(&existing.len()) && existing.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Some(existing);
        }
    }
    let mut bytes = [0u8; 16];
    window.crypto().ok()?.get_random_values_with_u8_array(&mut bytes).ok()?;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let id = format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]);
    storage.set_item(STORAGE_KEY, &id).ok()?;
    Some(id)
}

/// Always a factory, so a project with a real endpoint says what is missing (`backend.last_error`) instead of blaming the build.
/// The key and the device uuid are resolved when the factory runs (after `backend.ron` has loaded), not here.
pub fn backend_factory(params: Option<&web_sys::UrlSearchParams>) -> Option<BackendFactory> {
    let runner_key = params.and_then(|p| p.get("nakama_key")).filter(|k| !k.trim().is_empty());
    Some(ironhold_nakama::factory(RunnerOptions {
        runner_key,
        missing_key_hint: "no Nakama server key: set server_key in backend.ron or add ?nakama_key=... to the page address".to_string(),
        device_uuid: std::sync::Arc::new(|| {
            web_sys::window()
                .and_then(|w| device_uuid(&w))
                .ok_or_else(|| "the browser would not store a device id (private window or blocked site data?)".to_string())
        }),
    }))
}

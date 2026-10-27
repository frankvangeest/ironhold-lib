//! Wires the Nakama adapter into the web runner. The server key is a client identifier, not a secret (it ships in any web
//! build), so it comes from the page URL (`?nakama_key=`); the device uuid is generated once per browser profile and kept
//! in `localStorage`. Without a key a project with a real endpoint runs offline and says so.

use ironhold_core::runtime::backend::BackendFactory;
use ironhold_nakama::NakamaOptions;

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
pub fn backend_factory(params: &web_sys::UrlSearchParams) -> Option<BackendFactory> {
    let missing = |why: &'static str| Some(BackendFactory::new(move |_| Err(why.to_string())));
    let Some(server_key) = params.get("nakama_key").filter(|k| !k.trim().is_empty()) else {
        return missing("no Nakama server key: add ?nakama_key=... to the page address");
    };
    let Some(device_uuid) = web_sys::window().and_then(|w| device_uuid(&w)) else {
        return missing("the browser would not store a device id (private window or blocked site data?)");
    };
    Some(ironhold_nakama::factory(NakamaOptions { server_key, device_uuid }))
}

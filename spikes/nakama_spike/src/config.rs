//! Runtime configuration. Nothing here is committed: the endpoint comes from the environment and the Nakama server key
//! from the environment or from a local file OUTSIDE the repository, and it is never printed.

use std::path::PathBuf;

pub const DEFAULT_URL: &str = "http://192.168.178.106:7350";

pub struct Config {
    pub base_url: String,
    pub server_key: String,
}

fn key_file_path() -> PathBuf {
    if let Some(p) = std::env::var_os("NAKAMA_SERVER_KEY_FILE") {
        return PathBuf::from(p);
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default();
    PathBuf::from(home).join(".nakama_spike_server_key")
}

impl Config {
    /// `NAKAMA_URL` (default: the homelab stage 1 endpoint) and the server key from `NAKAMA_SERVER_KEY` or the key file.
    pub fn load() -> Result<Config, String> {
        let base_url = std::env::var("NAKAMA_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());
        let server_key = match std::env::var("NAKAMA_SERVER_KEY") {
            Ok(k) if !k.trim().is_empty() => k.trim().to_string(),
            _ => {
                let path = key_file_path();
                std::fs::read_to_string(&path)
                    .map(|k| k.trim().to_string())
                    .map_err(|e| format!("no server key: set NAKAMA_SERVER_KEY or create {} ({e})", path.display()))?
            }
        };
        if server_key.is_empty() {
            return Err("the server key is empty".into());
        }
        Ok(Config { base_url: base_url.trim_end_matches('/').to_string(), server_key })
    }
}

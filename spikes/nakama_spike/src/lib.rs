//! THROWAWAY spike (planning/features/nakama_backend_spike.md): a hand-written Nakama client in two layers.
//!
//! - `protocol`: pure message types and parsing, compiled for native and wasm32.
//! - `native`: the native transport (tokio + tokio-tungstenite). The browser transport is `ewebsock`, used from the wasm page;
//!   it is not in this library yet.

pub mod protocol;
pub mod relay;

#[cfg(target_arch = "wasm32")]
pub mod web;
#[cfg(target_arch = "wasm32")]
pub mod web_relay;
#[cfg(not(target_arch = "wasm32"))]
pub mod config;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;

/// Size probe for the protobuf comparison (S8): pulls the encode and decode paths into the wasm so the delta against the JSON-only
/// build is realistic (an unused dependency would be dropped by the linker).
#[cfg(all(target_arch = "wasm32", feature = "proto"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn proto_probe(match_id: String, payload: Vec<u8>) -> usize {
    use protocol::proto::{Envelope, MatchDataSend};
    let bytes = Envelope::data(MatchDataSend { match_id, op_code: 2, data: payload, presences: vec![], reliable: true }).to_bytes();
    Envelope::parse(&bytes).map(|e| e.cid.len()).unwrap_or(0) + bytes.len()
}

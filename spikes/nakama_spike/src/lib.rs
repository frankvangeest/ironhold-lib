//! THROWAWAY spike (planning/features/nakama_backend_spike.md): a hand-written Nakama client in two layers.
//!
//! - `protocol`: pure message types and parsing, compiled for native and wasm32.
//! - `native`: the native transport (tokio + tokio-tungstenite). The browser transport is `ewebsock`, used from the wasm page;
//!   it is not in this library yet.

pub mod protocol;

#[cfg(target_arch = "wasm32")]
pub mod web;
#[cfg(not(target_arch = "wasm32"))]
pub mod config;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;

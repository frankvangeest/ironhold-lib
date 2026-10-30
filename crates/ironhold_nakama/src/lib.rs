//! Nakama adapter for Ironhold's online backend contract (`planning/features/backend_abstraction.md`, v2).
//!
//! Request/response only (sign-in, leaderboard submit and list): every call is an `ehttp` request whose callback pushes the
//! answer into a channel that `poll()` drains. No thread of our own, no async runtime, the same code on native and wasm32.
//! The realtime socket (matchmaker, reconnect) is v3.
//!
//! The runner builds a [`RunnerOptions`] (an optional server key from outside the project, and a source for the per-install
//! device uuid, which only the runner can persist) and hands [`factory`] to `AppOptions::backend_factory`; the engine calls
//! it once `backend.ron` resolves, and the server key then comes from `backend.ron`'s `server_key` first.

pub mod ns;
pub mod server_config;

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
mod protocol;

pub use ns::{GameNs, NsLeaderboardId};
#[cfg(feature = "client")]
pub use client::{factory, resolve_server_key, DeviceUuidSource, NakamaBackend, NakamaOptions, RunnerOptions};

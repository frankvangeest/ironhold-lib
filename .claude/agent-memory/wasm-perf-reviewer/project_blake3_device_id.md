---
name: blake3-device-id
description: ironhold_nakama direct blake3 dep (device_id_binding, 2026-11-08) flips blake3 "std" on (bevy_asset uses default-features=false); harmless on wasm32
metadata:
  type: project
---

ironhold_nakama took `blake3 = "1.8"` as a direct dep (feature/device_id_binding, reviewed 2026-11-08). bevy_asset 0.18 pulls blake3 with `default-features = false`, so the direct dep's default turns on `std` for the whole graph (verified with `cargo tree -e features -i blake3 --target wasm32-unknown-unknown --offline`). `std` only gates `impl Error for HexError`, `io::Write`/`update_reader`, and x86 runtime feature detection — dead/unreachable on wasm32. wasm32 SIMD stays off (needs the opt-in `wasm32_simd` feature), no rayon, no mmap. Size impact ~0 (portable compression code already linked via bevy_asset).

GameNs::device_id runs once per sign-in (factory call), not per frame; the per-byte `format!` hex is a nit only.

**How to apply:** if someone adds `features = ["rayon"|"mmap"|"wasm32_simd"]` or similar to blake3 in any crate, re-check — features unify across bevy too. `cargo tree --offline` is cheap and safe (no compile). See [[project_wasm_size]], [[project_nakama_adapter]].

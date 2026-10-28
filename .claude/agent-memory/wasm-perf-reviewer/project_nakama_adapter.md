---
name: project-nakama-adapter
description: Backend v2 Nakama adapter (crates/ironhold_nakama) web cost - ehttp wasm graph verified, poll idle is alloc-free, startup localStorage only with ?nakama_key
metadata:
  type: project
---

Reviewed feature/nakama_adapter 2026-10-28. Verdict OK for web build.

- ehttp 0.7 target-gates ureq (and so rustls/ring/getrandom 0.2) to non-wasm32; verified with
  `cargo tree --offline -p ironhold_web --target wasm32-unknown-unknown -i ureq` -> nothing. `cargo tree --offline`
  needs no build and is safe to run while another cargo job runs.
- wasm-bindgen-futures, base64 0.22, serde_json are ALREADY in the web graph via bevy_gltf/gltf-json/winit, so the
  real new web code is ehttp's fetch glue + serde_json::Value/json! + the adapter (estimated well under 200 KB pre-opt).
- NakamaBackend::poll on idle = one std mpsc try_recv (no alloc) + Vec::append of an empty Vec (no alloc).
  backend_glue_system is run_if(backend_is_ready); see [[project-backend-glue-system]].
- Per request: Sender clone + boxed closure + spawn_local future + header/body copies; request-rate only.
- Web runner touches localStorage/crypto ONLY when `?nakama_key=` is present (short-circuits before window access).
- Latent: hub timeout does not cancel the ehttp fetch; a hung request's Pending entry + closure live until the
  browser's own fetch timeout. Bounded, not per-frame.

**How to apply:** don't re-flag the ureq/rustls lockfile entries as web bloat; they are native-only.

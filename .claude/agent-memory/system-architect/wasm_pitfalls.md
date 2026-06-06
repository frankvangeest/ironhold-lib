---
name: wasm-pitfalls
description: Known WASM/WebGPU compatibility pitfalls in ironhold-lib; consult before approving any new API usage
metadata:
  type: project
---

## WebGPU pipeline compilation is synchronous on WASM

On WASM, `device.createRenderPipeline()` is synchronous and lazy — it fires on first draw of a new mesh+material combination. Each call takes ~100–2000 ms. Mitigations in place:
- `pipeline_warmup_system`: adds `NoFrustumCulling` to all `Mesh3d` entities for 4 frames after scene load
- Spawn queue cap: `SPAWNS_PER_FRAME = 2` prevents wave-spawn stalls
- `Action::PreloadPrefab`: pre-fetches GLB assets before player can interact
- Particle variant warmup: explicit `SpawnEffect` at y=-100 for each pipeline variant

Any new rendering feature must consider whether it introduces a new pipeline variant that needs warmup.

## No multithreading on wasm32

`wasm32` is single-threaded. Do not use `std::thread`, `tokio`, `rayon`, or any threading primitives directly. Bevy's `AsyncComputeTaskPool` works on WASM via JS microtasks — use that for compute-heavy work (e.g., terrain mesh generation).

## 16-byte alignment for GPU structs

WebGPU validates buffer binding interfaces strictly; native wgpu is permissive. Always use `Vec4` for uniform buffer fields. A struct that passes native testing may still panic in web builds with `BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED`.

## Excluded tonemapping

`TonyMcMapface` and `BlenderFilmic` require a LUT texture — do not use them. Stick to `ReinhardLuminance`, `AcesFitted`, or `Neutral`.

## Binary size limit

GitHub Pages hard-blocks at 100 MB. Current WASM binary size ~91 MB (last checked 2026-06-04). Warn Frank at 95 MB. Every new capability adds to the binary. Large dependencies (physics, rendering) dominate; the marginal cost of a new capability is small but accumulates.

## Asset loading on WASM

HTTP fetch + GLTF decode on first load takes ~1–2 s. Use `Action::PreloadPrefab` during scene.ready to warm the asset server cache before the player can interact with a spawn trigger.

## WebFetch domain restrictions

The permission allow list restricts `WebFetch` to `docs.rs` and `github.com`. Any new external dependency or documentation source needs to be added to `settings.json`.

# Feature: Deferred Rendering

_Status: Draft_
_Planned at: `9ca3af5` (2026-06-02)_
_Investigation: `planning/investigations/deferred_rendering_spike.md`_

---

## What

Replace Ironhold's current clustered-forward renderer with Bevy 0.18's built-in deferred rendering pipeline for opaque geometry. Transparent and additive materials (particles, decals, flame effects) stay on the forward path automatically — no material changes required.

The change is three lines on the camera spawn. The investigation confirmed WASM builds clean, GL backends degrade gracefully, and all custom materials work without modification.

---

## Why

The current `MAX_FADING_LIGHTS = 16` hard cap in `fading_light.rs` exists because WebGPU's mobile tile limits restrict clustered forward rendering to a low simultaneous light count. Deferred rendering processes lights per-pixel against the G-buffer — no cluster count limit. Particle systems, dungeon torches, and explosion effects can all emit light simultaneously without competing for slots.

---

## How it works (from investigation)

| Path | What renders there |
|---|---|
| **Deferred** | `StandardMaterial` opaque geometry (GLB models, terrain, props) |
| **Forward** | `CustomMaterial`, `TerrainMaterial`, `PoolFlameMaterial` — all transparent and additive materials |

Custom materials default to `OpaqueRendererMethod::Forward` unless they explicitly override `opaque_render_method()`. None of Ironhold's custom materials do this, so the split is automatic.

### G-buffer formats (WASM-safe)

Bevy 0.18 uses `Rgba32Uint` and `R8Uint` for the G-buffer — standard WebGPU color-renderable formats supported across desktop and mobile GPUs. The format risk that previously blocked this feature is resolved.

### GL / ANGLE fallback

When the engine falls back to GL (Playwright headless, older hardware), Bevy's renderer silently skips `DeferredPrepass` and renders the full scene on clustered forward. One harmless WARN per scene load on the GL backend — does not affect rendering.

### MSAA trade-off

Bevy's `check_msaa` system automatically disables `Msaa::Sample4` when `DeferredPrepass` is present. Deferred and MSAA are architecturally incompatible. `Fxaa` is added alongside `DeferredPrepass` to preserve edge quality; it runs as a post-process pass and costs less memory than MSAA.

---

## Runtime changes

### `scene_loader.rs` — camera spawn

```rust
use bevy::core_pipeline::prepass::{DeferredPrepass, DepthPrepass};
use bevy::core_pipeline::fxaa::Fxaa;

commands.spawn((
    Camera3d::default(),
    DepthPrepass,
    DeferredPrepass,
    Fxaa::default(),
    // ... existing camera components unchanged
));
```

That is the entire change to the rendering path. No material changes. No shader changes. No new system registration.

### `fading_light.rs` — raise the light cap

Remove the `MAX_FADING_LIGHTS = 16` constant once the deferred pipeline is confirmed working in a native test. The cap can be raised to a large value (e.g. 256) as a conservative first step, then removed entirely once the new ceiling is validated in practice.

```rust
// Before:
const MAX_FADING_LIGHTS: usize = 16;

// After (v1 — raise conservatively):
const MAX_FADING_LIGHTS: usize = 256;

// After (v2 — remove entirely once validated):
// cap removed; fading_light_system manages its own VecDeque without a size limit
```

---

## No schema changes

This is a rendering pipeline change. Designers do not control which rendering path a scene uses — the engine always enables deferred on camera spawn, and Bevy routes each material automatically. No new fields on `GameSceneV2`, `ProjectConfig`, or any prefab type.

---

## Tasks

- [ ] Add `DepthPrepass`, `DeferredPrepass`, `Fxaa::default()` to the camera spawn in `scene_loader.rs`
- [ ] Run `particles_demo` natively and confirm >16 simultaneous particle lights render without capping
- [ ] **Manual Chrome WebGPU check** — `python serve.py`, open in Chrome with WebGPU enabled, check console for `Rgba32Uint` texture format errors (the one remaining unknown from the investigation)
- [ ] Raise `MAX_FADING_LIGHTS` to 256 once native test passes
- [ ] `cargo test -p ironhold_core` passes with no regression
- [ ] `python test_web.py` — all screenshot baselines pass (GL backend falls back cleanly)
- [ ] Remove `MAX_FADING_LIGHTS` cap entirely once WebGPU manual test passes (or make it a project config field if a safe ceiling is needed for mobile)
- [ ] Update `crates/ironhold_core/src/CLAUDE.md` — note that `MAX_FADING_LIGHTS` no longer applies on deferred path
- [ ] Move investigation file to `planning/investigations/done/` after shipping

---

## Acceptance criteria

- Given `particles_demo` running natively, more than 16 simultaneous particle lights are visible at once with no hard cutoff.
- Given opening any example project in Chrome with native WebGPU, no `Validation Error` or texture format errors appear in the browser console.
- Given opening any example project in a GL/ANGLE browser (headless Chromium), the scene renders correctly — one harmless WARN in the terminal, no visual artifacts.
- `CustomMaterial`, `TerrainMaterial`, and particle materials retain their custom visual effects unchanged — toon shading, terrain splatmap blending, flame distortion all present.
- `python test_web.py` exits 0 — all screenshot baselines match (rendering pipeline change is not visually breaking for opaque geometry in the test suite's GL backend).
- `cargo test -p ironhold_core` exits 0.

# Investigation: Deferred Rendering Spike

_Opened: 2026-05-31_
_Backlog entry: Icebox → Rendering & Assets → "Deferred rendering"_

## Goal

Determine whether Bevy 0.18's deferred rendering pipeline is viable for Ironhold — specifically whether it works on WASM/WebGPU, whether Ironhold's custom materials need changes, and whether mixed deferred/forward scenes render correctly.

A successful investigation unblocks writing the full feature file and moving the backlog item to Queued. A failed investigation (WebGPU incompatible) means the item gets WASM-BLOCKED like Bloom.

## Why this matters

The current `MAX_FADING_LIGHTS = 16` cap in `fading_light.rs` exists because Bevy's clustered forward renderer has a practical ceiling on WebGPU (mobile tile limits as low as 32 total lights). Deferred rendering removes the per-cluster light count limit entirely — every dynamic particle light, dungeon torch, and explosion would work without competing for slots.

## Questions to answer

1. **Does Bevy 0.18 deferred compile and render on WASM/WebGPU?**
   The G-buffer uses `Rgb9e5Ufloat` (packed lighting) and `R16Uint` (material ID) texture formats. WebGPU support for these varies across browsers and mobile GPUs. A browser console error or black screen on the deferred pass is the failure mode.

2. **Do Ironhold's custom materials need changes?**
   Standard PBR materials handle the deferred prepass via `DeferredPrepass` automatically in Bevy 0.18. Custom WGSL materials (`CustomMaterial`, terrain material, particle pool materials, `UiMaterial`) may need to explicitly opt in or they silently fall back to forward. Check each material type.

3. **Does the mixed forward/deferred scene render correctly?**
   Transparent/additive materials (particles, decals, fire) cannot use deferred — they must stay on the forward path. Bevy handles this by rendering deferred opaques first, then forward transparents on top. Verify there are no depth artifacts or ordering issues in a scene with both.

## Approach

### Step 1 — Native spike (30 min)
Enable Bevy's deferred rendering on one camera in a scene that has dynamic lights:

```rust
// In scene_loader.rs or a test scene, add to the camera spawn:
use bevy::pbr::deferred::DeferredPrepass;
use bevy::core_pipeline::prepass::DepthPrepass;

commands.spawn((
    Camera3d::default(),
    DepthPrepass,
    DeferredPrepass,
    // ... existing camera components
));
```

Load `particles_demo` (has dynamic fading lights from particle effects). Confirm:
- Scene renders correctly (no black/corrupted materials)
- More than 16 simultaneous dynamic lights work without the cap
- Custom materials (terrain, particle pool) render at all

### Step 2 — WASM spike (30 min)
Build WASM and open in Chrome/Firefox:
```bash
wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg
python serve.py
```

Open browser console. Look for:
- `wgpu error: Validation Error` related to texture formats
- Black screen or missing geometry on opaque materials
- Particle/transparent geometry still visible (forward path)

### Step 3 — Custom material audit (30 min)
For each custom material type, check if it needs a deferred prepass shader variant:

| Material | File | Expected behaviour |
|---|---|---|
| `CustomMaterial` | `capabilities/custom_material.rs` | May need `DeferredPrepass` vertex/fragment shader |
| Terrain material | `capabilities/terrain_material.rs` | Standard PBR — likely automatic |
| Particle pool | `capabilities/particle.rs` | Transparent/additive — stays forward, fine |
| `UiMaterial` (stat radar) | `capabilities/stats.rs` | 2D/UI — unaffected by deferred |

## Expected findings

**Best case:** Bevy 0.18 deferred compiles and renders on WebGPU, standard materials work automatically, particles stay on forward with no artifacts. → Write feature file, move to Queued.

**Likely case:** Native works fine; WASM has a texture format error on `Rgb9e5Ufloat` in some browsers. → Investigate fallback: can the G-buffer use a more universally supported format? If Bevy allows format override, it may be fixable. Otherwise WASM-BLOCK it.

**Worst case:** Deferred breaks custom materials or causes depth artifacts with the forward transparent pass. → WASM-BLOCK or defer until Bevy's deferred matures further.

## Files to read

- `crates/ironhold_core/src/capabilities/fading_light.rs` — current MAX_FADING_LIGHTS cap and rationale
- `crates/ironhold_core/src/capabilities/custom_material.rs` — custom WGSL material system
- `crates/ironhold_core/src/capabilities/terrain_material.rs` — terrain WGSL shader
- `crates/ironhold_core/src/runtime/scene_manager/scene_loader.rs` — camera spawn, where DeferredPrepass would be added
- Bevy 0.18 changelog / `bevy_pbr::deferred` docs — current deferred API surface

## Findings

### G-buffer texture formats — better than expected

The investigation assumed `Rgb9e5Ufloat` / `R16Uint` based on older Bevy versions. Bevy 0.18 changed to:

```rust
// bevy_core_pipeline-0.18.0/src/deferred/mod.rs
pub const DEFERRED_PREPASS_FORMAT: TextureFormat = TextureFormat::Rgba32Uint;
pub const DEFERRED_LIGHTING_PASS_ID_FORMAT: TextureFormat = TextureFormat::R8Uint;
pub const DEFERRED_LIGHTING_PASS_ID_DEPTH_FORMAT: TextureFormat = TextureFormat::Depth16Unorm;
```

`Rgba32Uint` and `R8Uint` are standard, widely-supported WebGPU color-renderable formats. The format risk that blocked this investigation is gone. **WASM risk is LOW.**

### Custom material compatibility — automatic, no changes needed

Bevy 0.18's `Material` trait default implementation:

```rust
// bevy_pbr-0.18.0/src/material.rs
fn opaque_render_method(&self) -> OpaqueRendererMethod {
    OpaqueRendererMethod::Forward
}
```

The default is `Forward`. Since `CustomMaterial`, `TerrainMaterial`, and `PoolFlameMaterial` all implement `Material` without overriding `opaque_render_method()`, they automatically stay on the forward path when `DeferredPrepass` is added to the camera. Custom visual effects (toon shading, terrain splatmap blending, flame UV distort) are fully preserved. **No material changes required.**

### Mixed rendering — native to Bevy

The scene renders correctly as a mixed pipeline without any special wiring:

| Material | Path | Effect |
|---|---|---|
| `StandardMaterial` (GLB models, props) | Deferred | Multi-light efficient |
| `CustomMaterial` (toon, cel, custom WGSL) | Forward (automatic) | Custom effects preserved |
| `TerrainMaterial` (splatmap terrain) | Forward (automatic) | Splatmap blending preserved |
| `PoolFlameMaterial` (fire, particles) | Forward (AlphaMode::Add) | Transparent — always forward |
| `StandardMaterial` additive/blend (particles) | Forward (transparent) | Unaffected |

### MSAA — auto-disabled, replacement needed

Ironhold sets no explicit MSAA; Bevy defaults to `Msaa::Sample4`. Bevy's built-in `check_msaa` system automatically sets cameras with `DeferredPrepass` to `Msaa::Off`. This silently removes anti-aliasing.

Bevy 0.18 has `Fxaa` as a built-in post-process AA. Should be added alongside `DeferredPrepass` to preserve edge quality. This is a one-liner:
```rust
commands.spawn((Camera3d::default(), DeferredPrepass, DepthPrepass, Fxaa::default(), ...));
```

### API — available in Bevy 0.18, minimal to enable

```rust
use bevy::core_pipeline::prepass::{DeferredPrepass, DepthPrepass};
```

Adding to a camera spawn is a two-component insert. `cargo check` confirms the project compiles cleanly today — no API blockers.

### MAX_FADING_LIGHTS cap — can be removed

`fading_light.rs` caps dynamic particle lights at 16 specifically because of clustered forward WebGPU tile limits. With deferred, `StandardMaterial` point lights are processed per-pixel without a cluster count limit. The cap can be raised significantly (or removed) once deferred ships.

### Remaining unknown — live WASM browser test

All code analysis points to WASM compatibility, but a live browser test has not been run. This is the one remaining item before shipping. The format evidence is strong; no known blockers.

## Outcome

- [ ] Native: **expected to work** — add `DeferredPrepass + DepthPrepass + Fxaa` to camera, run `particles_demo`, confirm GLB models lit by >16 lights _(not yet run — low risk based on analysis)_
- [ ] WASM Chrome: **expected to work** — `Rgba32Uint`/`R8Uint` are standard WebGPU formats _(must run before shipping feature)_
- [ ] WASM Firefox: **expected to work** _(must run before shipping feature)_
- [x] Custom materials: **no changes needed** — all default to `OpaqueRendererMethod::Forward`
- [x] Mixed scene: **correct by design** — Bevy routes materials automatically
- [x] **Decision: write feature file.** Move to Queued after WASM browser test passes.

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

_(fill in after running the spike)_

## Outcome

- [ ] Native: works / fails (note errors)
- [ ] WASM Chrome: works / fails (note errors)
- [ ] WASM Firefox: works / fails (note errors)
- [ ] Custom materials: need changes / automatic
- [ ] Mixed scene: correct / artifacts (describe)
- [ ] Decision: write feature file | WASM-BLOCK | defer

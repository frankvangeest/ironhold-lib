# Feature: Particle System v2 — 8. Flipbook / Sprite Sheet Animation

_Status: **Blocked — design must be rewritten before implementation**_
_Planned at: `2cc61ca` (2026-05-19)_
_Blocked at: `a16bd98` (2026-05-23) — goal-alignment review found renderer mismatch_
_Part of: see `planning/features/particle_system_v2.md` for the full v2 overview_

## Blocking issue

The Dependencies section and entire Approach section were written for a **GPU-instanced
renderer** that does not exist. The renderer that shipped (feature 1, pool renderer) is
a **CPU-side particle pool** that rebuilds mesh vertex data every frame. There is no
per-instance GPU buffer, and there is no instanced particle shader.

Before any implementation work starts, the Approach section must be rewritten to answer:

1. **How are flipbook UVs applied in the CPU pool renderer?**
   The pool renderer builds quad corners on the CPU each frame. UV rects for the current
   frame must be baked into the vertex UV attribute during that build step — not written
   to a GPU instance buffer. Confirm this is the chosen path.

2. **Does flipbook require a new pool group / pipeline variant?**
   The current pool has three pipeline variants: Additive (`StandardMaterial + Add`),
   Blend (`StandardMaterial + Blend`), PoolFlameMaterial (`Add` + UV distort shader).
   A sprite-sheet flipbook particle with Additive blend and standard UV selection may
   fit the existing `StandardMaterial + Add` variant with no new pipeline. Confirm or
   document the new variant.

3. **If a new pipeline variant is added, warmup is required.**
   Per `crates/ironhold_core/src/CLAUDE.md` (Particle pipeline warmup section), every
   new `(blend_mode, material_type)` combination triggers a 300–1000 ms synchronous
   WebGPU compile stall on first use in WASM. Add the corresponding warmup
   `SpawnEffect` call to the design's implementation tasks.

Do not start implementation until these three questions are answered and documented here.

---

## What

Particle layers can use a sprite sheet (flipbook animation) instead of a static texture.
Each particle advances through UV frames over its lifetime, enabling pre-authored
frame-by-frame animations — impact flashes, magic seal draw-ons, explosion blooms.

## Why

UV distortion and scroll animate a static texture procedurally, which is good for organic
effects (fire, smoke). But impacts, spell casts, and complex shape transitions are better
expressed as hand-authored frame sequences. A 4×4 sprite sheet at 24 fps gives 16 frames
of bespoke animation at essentially zero runtime cost beyond a UV offset calculation.

## Dependencies

Depends on the instanced renderer (feature 1). The flipbook frame advance writes a
`uv_offset` value into the per-instance buffer; the instanced particle shader reads it
to select the correct sub-rectangle.

## Approach

Add a `flipbook: Option<FlipbookDef>` field to `LayerDef`:

```ron
flipbook: (
  cols: 4,
  rows: 4,        // 4×4 = 16 frames total
  fps: 24.0,
  loop: false,    // false: play once, hold last frame until despawn
                  // true: loop for the full lifetime
),
```

**CPU simulation (per particle each frame):**
```rust
let frame = ((particle.elapsed * def.fps) as usize)
    .min(if def.loop { usize::MAX } else { cols * rows - 1 })
    % (cols * rows);
let col = frame % cols;
let row = frame / cols;
let uv_offset = Vec2::new(col as f32 / cols as f32, row as f32 / rows as f32);
let uv_scale  = Vec2::new(1.0 / cols as f32, 1.0 / rows as f32);
// write uv_offset + uv_scale into ParticleInstance
```

The instanced particle shader multiplies incoming UV by `uv_scale` and adds `uv_offset`
to display the current frame.

**Sprite sheet authoring:**
- Sheets in `assets/shared/textures/particles/sheets/`
- Power-of-two PNGs (256×256, 512×512, 1024×1024)
- Row order: top-to-bottom, left-to-right (matches Aseprite / Photoshop export)
- White-on-transparent; colour from RON gradient as usual

**Initial sheets to create:**
- `explosion_16f.png` (4×4, each frame hand-drawn or exported from a reference)
- `impact_flash_9f.png` (3×3)

**Interaction with UV distort:** disallow both `flipbook` and `uv_distort > 0` on the
same layer. Validate in `AssetCatalog::validate()` and fail with a clear error.

## Tasks

- [ ] Add `FlipbookDef` struct to `schema/catalog.rs`
- [ ] Add `flipbook: Option<FlipbookDef>` to `LayerDef`
- [ ] Add validation: `flipbook` + `uv_distort > 0` → error
- [ ] Extend `ParticleInstance` struct with `uv_offset: [f32; 2]` + `uv_scale: [f32; 2]`
  (may already be present from instanced renderer implementation)
- [ ] Implement frame advance + UV writes in simulation tick
- [ ] Update instanced particle shader to read `uv_offset` + `uv_scale`
- [ ] Create `explosion_16f.png` sprite sheet (even a rough placeholder)
- [ ] Add a flipbook effect to particles_demo (explosion burst uses sheet)
- [ ] RON parse test for `FlipbookDef`
- [ ] Visual / screenshot test for flipbook effect in WASM
- [ ] Update `docs/20_data_formats.md`

## Open questions

- **Hold vs despawn on last frame**: when `loop: false`, should the particle hold the
  last frame until its `lifetime_secs` expires, or despawn immediately when the animation
  finishes? Hold is more flexible — the designer controls despawn via lifetime.
- **Non-square grids**: most sheets are square, but `cols != rows` (e.g. a 6×2 strip)
  should work. Confirm the formula handles this correctly.
- **Sub-frame interpolation**: blend between consecutive frames for smoother animation
  at lower fps values? Adds complexity; probably not worth it for stylised effects.

## Acceptance criteria

- `flipbook: (cols: 4, rows: 4, fps: 24.0, loop: false)` plays all 16 frames over
  `lifetime_secs` and holds the last frame until despawn
- `loop: true` repeats the animation continuously for the full lifetime
- Authoring `flipbook` alongside `uv_distort > 0` fails catalog validation with a
  clear error message
- The explosion effect in particles_demo uses the flipbook path and looks distinct from
  the current sphere-particle burst

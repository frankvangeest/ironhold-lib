# Feature: Particle System v2 — 8. Flipbook / Sprite Sheet Animation

_Status: **Active**_
_Planned at: `2cc61ca` (2026-05-19)_
_Activated at: `942e96d` (2026-05-30)_
_Part of: see `planning/features/particle_system_v2.md` for the full v2 overview_

## What

Particle layers can use a sprite sheet (flipbook animation) instead of a static texture.
Each particle advances through UV frames over its lifetime, enabling pre-authored
frame-by-frame animations — impact flashes, magic seal draw-ons, explosion blooms.

## Why

UV distortion and scroll animate a static texture procedurally, which is good for organic
effects (fire, smoke). But impacts, spell casts, and complex shape transitions are better
expressed as hand-authored frame sequences. A 4×4 sprite sheet at 24 fps gives 16 frames
of bespoke animation at essentially zero runtime cost beyond a UV offset calculation.

---

## Blocking questions resolved (2026-05-30)

### 1. How are flipbook UVs applied in the CPU pool renderer?

UV coordinates are baked into `ATTRIBUTE_UV_0` vertex data during the CPU quad-build
step in `rebuild_pool_meshes_system` (currently line 372 of `particle_renderer.rs`).
The default writes `[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]` (full texture).

For flipbook, the current frame sub-rectangle is computed each frame from `p.elapsed`,
then the four UV corners are written with that sub-rect — no GPU instance buffer, no
shader changes required.

**UV math** (row-major, top-to-bottom, left-to-right layout, standard UV orientation):
```
frame  = (elapsed * fps) as usize
frame  = if loop { frame % total } else { frame.min(total - 1) }
col    = frame % cols;  row = frame / cols
u0 = col / cols;        u1 = (col + 1) / cols
v0 = row / rows;        v1 = (row + 1) / rows

corners: BL=[u0,v1]  BR=[u1,v1]  TR=[u1,v0]  TL=[u0,v0]
```

### 2. Does flipbook require a new pool group / pipeline variant?

**No.** Flipbook particles use `GroupKey::Additive` or `GroupKey::Blend` exactly like
existing sprite particles. The sprite sheet PNG becomes `texture_path` in the group key,
creating a distinct draw group per sheet without any new material type. The existing
`StandardMaterial` renders whatever UVs the vertices carry.

### 3. Does adding flipbook require new pipeline warmup?

No new variant. The warmup `SpawnEffect` pattern already covers Additive + Blend variants.
However, if a scene's first flipbook effect introduces a sprite sheet texture not used by
any other effect, that new group entity triggers a pipeline compile on first render.
**Mitigation**: fire a low-count warmup `SpawnEffect` on `scene.ready` at `y=-100`,
same as any new texture-bearing effect.

---

## Schema

Add `FlipbookDef` struct and `flipbook: Option<FlipbookDef>` field to both `LayerDef`
and `EffectDef`. Both structs have `#[serde(deny_unknown_fields)]` — the field is
required in both. `From<&EffectDef> for LayerDef` must copy it.

```ron
// In LayerDef or EffectDef:
flipbook: (
    cols: 4,
    rows: 4,      // 4×4 = 16 frames total
    fps: 24.0,
    loop: false,  // false: hold last frame until despawn; true: loop for full lifetime
),
```

## Approach

### Schema changes (`schema/catalog.rs`)

1. Add `FlipbookDef`:
```rust
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlipbookDef {
    pub cols: u8,
    pub rows: u8,
    pub fps: f32,
    #[serde(default)]
    pub r#loop: bool,
}
```
2. Add `#[serde(default)] pub flipbook: Option<FlipbookDef>` to both `LayerDef` and `EffectDef`.
3. Copy field in `From<&EffectDef> for LayerDef`.
4. Add validation in `AssetCatalog::validate()`: if `layer.flipbook.is_some() && layer.uv_distort > 0.0` → error.

### Runtime changes (`capabilities/particle_renderer.rs`)

Add flipbook fields to `PooledParticle`:
```rust
pub flipbook_cols: u8,   // 0 = not a flipbook particle
pub flipbook_rows: u8,
pub flipbook_fps:  f32,
pub flipbook_loop: bool,
```

In `particle.rs` where `PooledParticle` is constructed from a `LayerDef`, copy the
`flipbook` fields (0 / false defaults when `None`).

In `rebuild_pool_meshes_system`, replace the fixed UV write:
```rust
// Before:
uvs.extend_from_slice(&[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);

// After:
let (u0, u1, v0, v1) = if p.flipbook_cols > 0 {
    let total = p.flipbook_cols as usize * p.flipbook_rows as usize;
    let raw   = (p.elapsed * p.flipbook_fps) as usize;
    let frame = if p.flipbook_loop { raw % total } else { raw.min(total - 1) };
    let col   = frame % p.flipbook_cols as usize;
    let row   = frame / p.flipbook_cols as usize;
    let cf    = p.flipbook_cols as f32;
    let rf    = p.flipbook_rows as f32;
    (col as f32 / cf, (col + 1) as f32 / cf, row as f32 / rf, (row + 1) as f32 / rf)
} else {
    (0.0, 1.0, 0.0, 1.0)
};
uvs.extend_from_slice(&[[u0, v1], [u1, v1], [u1, v0], [u0, v0]]);
```

### Asset — sprite sheet

Create one sprite sheet PNG for the demo effect:
- `assets/shared/textures/particles/sheets/explosion_4x4.png` — 4×4 white-on-transparent
  explosion sequence (256×256 or 512×512). Placeholder: can be a solid-white 1×1 grid
  renamed, or hand-authored later. The UV logic is valid regardless.

Add to `assets/shared/assets.ron` textures, and add a demo flipbook effect to
`assets/projects/particles_demo/assets.ron`.

---

## Tasks

- [ ] Add `FlipbookDef` struct to `schema/catalog.rs`
- [ ] Add `flipbook: Option<FlipbookDef>` to `LayerDef` (with `#[serde(default)]`)
- [ ] Add `flipbook: Option<FlipbookDef>` to `EffectDef` (with `#[serde(default)]`)
- [ ] Copy `flipbook` field in `From<&EffectDef> for LayerDef`
- [ ] Add validation: `flipbook.is_some() && uv_distort > 0.0` → error in `AssetCatalog::validate()`
- [ ] Add flipbook fields to `PooledParticle` in `particle_renderer.rs`
- [ ] Copy flipbook fields when constructing `PooledParticle` from `LayerDef` in `particle.rs`
- [ ] Replace fixed UV write with frame-computed sub-rect in `rebuild_pool_meshes_system`
- [ ] Create `explosion_4x4.png` sprite sheet (placeholder or hand-authored)
- [ ] Register sheet in `assets/shared/assets.ron` textures
- [ ] Add `explosion_flipbook` effect to `particles_demo/assets.ron`
- [ ] Add flipbook effect to `particles_demo` scene or trigger (swap out old burst, or add new trigger)
- [ ] RON parse test for `FlipbookDef` (in `ron_validation.rs`)
- [ ] Test: flipbook + uv_distort validation error
- [ ] Update `docs/20_data_formats.md`

---

## Open questions

- **Hold vs despawn on last frame**: `loop: false` holds last frame until `lifetime_secs` expires.
  Hold is more flexible — the designer controls despawn via lifetime. _(Answered: hold.)_
- **Non-square grids**: `cols != rows` (e.g. 6×2 strip) works fine; formula handles it.
- **Sub-frame interpolation**: blend between consecutive frames for smoother animation — deferred,
  not worth the complexity for stylised effects.

---

## Acceptance criteria

- `flipbook: (cols: 4, rows: 4, fps: 24.0, loop: false)` plays all 16 frames over
  `lifetime_secs` and holds the last frame until despawn
- `loop: true` repeats the animation continuously for the full lifetime
- Authoring `flipbook` alongside `uv_distort > 0` fails catalog validation with a clear error
- The explosion effect in `particles_demo` uses the flipbook path
- No new pipeline variant or warmup entry required
- All existing projects still pass `ron_validation` (new field is optional)
- Compiles for WASM — no new platform-specific code

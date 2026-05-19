# Feature: Particle System v2 — 3. Bloom / Post-Processing in Scene RON

_Status: Draft_
_Planned at: `2cc61ca` (2026-05-19)_
_Part of: see `planning/features/particle_system_v2.md` for the full v2 overview_

## What

`GameSceneV2` gains an optional `post_processing` block. When present, Bevy's built-in
bloom post-processing is enabled on the scene camera with the authored parameters.

## Why

Additive particles already push HDR brightness above 1.0 in linear light space, but
without a bloom pass that energy stays confined to the particle quads. Bloom spreads the
excess into a soft halo — fire looks hot, spells look magical. This is the single cheapest
visual improvement relative to authoring cost: one RON block, zero new Rust systems.

## Approach

Add `post_processing: Option<PostProcessingDef>` to `GameSceneV2`. Map to Bevy 0.18's
`BloomSettings` component, inserted on the `Camera3d` entity by `spawn_scene_v2`.

```ron
// In a scene RON
post_processing: (
  bloom: (
    intensity: 0.25,
    low_frequency_boost: 0.45,   // large bright areas bloom softly
    high_frequency_boost: 0.10,  // fine bright specks bloom sharply
    threshold: 0.75,             // pixels brighter than this bloom
    composite_mode: EnergyConserving,
  ),
),
```

Default: `post_processing` omitted → no `BloomSettings` inserted → identical to current
behaviour. All existing scene baselines are unaffected.

**New types in `schema/scenes.rs`:**

```rust
pub struct PostProcessingDef {
    pub bloom: Option<BloomDef>,
}

pub struct BloomDef {
    pub intensity: f32,
    pub low_frequency_boost: f32,
    pub high_frequency_boost: f32,
    pub threshold: f32,
    pub composite_mode: BloomCompositeModeRon,  // EnergyConserving | Additive
}
```

`BloomCompositeModeRon` is a thin RON-serialisable mirror of Bevy's `BloomCompositeMode`.

## Tasks

- [ ] Add `PostProcessingDef` + `BloomDef` structs to `schema/scenes.rs`
- [ ] Add `post_processing: Option<PostProcessingDef>` to `GameSceneV2`
- [ ] Insert `BloomSettings` on Camera3d in `spawn_scene_v2` when `bloom` is `Some`
- [ ] Add `post_processing` block to `particles_demo` scene RON
- [ ] Run `python test_web.py --update-baseline particles_demo` to record new baseline
- [ ] Add RON parse test for `PostProcessingDef`
- [ ] Benchmark bloom cost in WASM build (Chrome DevTools GPU timing)
- [ ] Update `docs/20_data_formats.md` — add `post_processing` to GameSceneV2 table

## Open questions

- **WASM performance**: bloom adds a multi-pass fullscreen resolve. On mobile WebGPU
  this may be 2–4 ms per frame. Measure before committing to enabling it in particles_demo.
  If too expensive, provide a `low_quality_bloom: true` flag that uses a single lower-res
  pass.
- **Bevy 0.18 field names**: verify `BloomSettings` struct fields haven't changed from
  recent Bevy versions before mapping them.

## Acceptance criteria

- A scene with `post_processing: (bloom: (...))` shows a visible glow halo around
  additive bright particles (campfire, torches) in the WASM build
- Omitting `post_processing` renders pixel-identically to current baselines
- All other scene baselines (`test_web.py`) pass unchanged
- Bloom adds ≤ 3 ms GPU time per frame in the WASM build (measured in Chrome)

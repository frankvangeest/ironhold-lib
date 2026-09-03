# Feature: Particle System v2 — 2. Multi-Layer EffectDef

_Status: Done_
_Planned at: `ff085be` (2026-05-19)_
_Part of: see `planning/features/particle_system_v2.md` for the full v2 overview_

## What

An `EffectDef` can define multiple emitter layers in a single RON definition. Designers
compose complex effects (campfire body + hot core) in one place instead of coordinating
multiple `SpawnEffect` calls across a behavior file and a rules file.

## Why

The current campfire fires `campfire_body` and `campfire_core` as two separate
`SpawnEffect` actions every 0.45 s. This scatters the definition across three files
(assets.ron × 2 entries, behavior × 2 actions, rules.ron × 2 warmup entries). Adding a
third layer — embers, say — means touching all three files again. The effect definition
belongs in one place.

## Approach

Extract `LayerDef` as a struct containing all existing flat `EffectDef` fields. Add
`layers: Vec<LayerDef>` to `EffectDef`. When `layers` is non-empty, flat fields are
ignored and each layer is spawned independently. This is fully backwards-compatible —
all existing single-layer effects continue to work with no changes.

```ron
// Before: two separate keys + two SpawnEffect calls in behavior
"campfire_body": ( ... ),
"campfire_core":  ( ... ),

// After: one key, one SpawnEffect call
"campfire": (
  layers: [
    ( // body
      sprites: ["particle/flame_01", ...],
      particle_count: 4,
      lifetime_secs: 1.0,
      ...
    ),
    ( // core
      sprites: ["particle/flame_05", "particle/flame_06"],
      particle_count: 2,
      lifetime_secs: 0.80,
      ...
    ),
  ],
  light: ( color: (1.0, 0.55, 0.15), intensity: 8000.0, range: 6.0, fade_out_secs: 0.5 ),
),
```

`drain_particle_effects_system` loops over layers when present. A single `SpawnEffect`
warmup entry covers all layers of that effect.

## Tasks

- [ ] Extract `LayerDef` struct from flat `EffectDef` fields in `schema/catalog.rs`
- [ ] Add `layers: Vec<LayerDef>` to `EffectDef`; document flat-fields-as-single-layer shorthand
- [ ] Update `drain_particle_effects_system` to iterate layers (or fall back to flat)
- [ ] Migrate `campfire_body` + `campfire_core` in `particles_demo/assets.ron` into one `"campfire"` key
- [ ] Update `campfire.behavior.ron` to a single `SpawnEffect(key: "campfire")` per tick
- [ ] Update `rules.ron` warmup to single `campfire` entry
- [ ] Update RON validation tests (add multi-layer parse + layer-count assertions)
- [ ] Update `docs/20_data_formats.md` — add `layers` to EffectDef table

## Open questions

- Should `LayerDef` also carry its own `light` block, or is light always top-level on
  `EffectDef`? Top-level is simpler; per-layer lights would let a multi-layer effect have
  different coloured lights per layer, which is rarely needed.
- When layers have different `lifetime_secs`, when does a top-level fading light
  consider the "effect" done? Use the maximum layer lifetime.

## Acceptance criteria

- `campfire.behavior.ron` fires one `SpawnEffect(key: "campfire")` per tick; no paired keys
- The campfire in particles_demo renders identically to the two-key version
- All existing single-layer `assets.ron` files parse and validate without modification
- A three-layer effect spawns all three layers at the correct position simultaneously

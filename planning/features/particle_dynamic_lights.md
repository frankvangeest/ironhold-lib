# Feature: Particle System v2 — 4. Dynamic Effect Lights

_Status: Draft_
_Planned at: `2cc61ca` (2026-05-19)_
_Part of: see `planning/features/particle_system_v2.md` for the full v2 overview_

## What

Effect definitions can include a `light` block. When the effect fires, a temporary
`PointLight` entity is spawned at the effect origin, fades in and out over the authored
durations, then despawns automatically. No extra action or system wiring is required
from the designer.

## Why

Every fire, explosion, and spell in stylised action games casts coloured light on nearby
geometry. Currently lights are static scene fixtures. Adding even one warm-orange light
near a campfire takes three steps (scene RON, correct position, tweak intensity). Effect
lights are the cheapest way to make bursts feel grounded — the designer just adds a `light`
block to the effect definition.

## Approach

Add `light: Option<EffectLightDef>` to the top-level `EffectDef` (and `LayerDef` once
multi-layer is implemented — for now top-level only).

```ron
"campfire": (
  // ... particle layers ...
  light: (
    color: (1.0, 0.55, 0.15),
    intensity: 8000.0,
    range: 6.0,
    fade_in_secs: 0.05,
    fade_out_secs: 0.40,
    // duration_secs omitted → matches the longest layer lifetime
  ),
),

"explosion_burst": (
  // ...
  light: (
    color: (1.0, 0.85, 0.40),
    intensity: 30000.0,
    range: 12.0,
    fade_in_secs: 0.0,
    fade_out_secs: 0.6,
  ),
),
```

**New component:**

```rust
#[derive(Component)]
pub struct FadingLight {
    pub peak_intensity: f32,
    pub fade_in_secs: f32,
    pub fade_out_secs: f32,
    pub duration_secs: f32,
    pub elapsed: f32,
}
```

**`fading_light_system` (Update):** ticks `elapsed`, computes the intensity envelope
(linear fade-in then linear fade-out), writes to `PointLight::intensity`, despawns at
`duration_secs`.

`drain_particle_effects_system` spawns a `PointLight` + `FadingLight` + `LevelEntity`
entity at the effect origin when `light` is `Some`. `LevelEntity` ensures it is cleaned
up on scene transitions.

**Files:**
- `capabilities/fading_light.rs` (new, ~60 lines)
- `schema/catalog.rs` — add `EffectLightDef`
- `runtime/scene_manager/action_executor.rs` — spawn light in `SpawnEffect` handling

## Tasks

- [ ] Add `EffectLightDef` struct to `schema/catalog.rs`
- [ ] Add `light: Option<EffectLightDef>` to `EffectDef`
- [ ] Implement `FadingLight` component + `fading_light_system` in `capabilities/fading_light.rs`
- [ ] Register `fading_light_system` in the app (after `drain_particle_effects_system`)
- [ ] Wire light spawn into `drain_particle_effects_system`
- [ ] Add `light` blocks to campfire, torch, explosion, and magic effects in particles_demo
- [ ] Add RON parse test for `EffectLightDef`
- [ ] Add integration test: SpawnEffect with light → PointLight entity exists → despawns at duration
- [ ] Update `docs/20_data_formats.md`

## Open questions

- **Light budget in raids**: 40 simultaneous casters may each spawn a light. 40 dynamic
  point lights may be expensive even with Bevy's clustered forward lighting. Should the
  particle budget (feature 7) also cap dynamic effect lights, or is Bevy's clustered
  lighting sufficient? Defer to benchmarking after implementation.
- **Per-layer lights**: once multi-layer EffectDef (feature 2) is implemented, should
  individual layers be able to carry their own light (e.g. two-colour fire with a hot
  white core light + warm orange body light)? Useful but deferred to v2.1.

## Acceptance criteria

- A campfire with `light: (...)` emits a visible warm-orange glow on the ground around it
- The explosion effect spawns a bright flash that visibly illuminates nearby geometry
  for ~0.6 s then disappears
- Effect lights are destroyed on scene load/transition (LevelEntity cleanup)
- Adding `light` to an effect does not change particle behaviour or timing

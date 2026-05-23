# Feature: Particle System v2 — 7. Quality Tiers & Particle Budget

_Status: Draft — reviewed, ready to implement_
_Planned at: `2cc61ca` (2026-05-19)_
_Reviewed at: `a16bd98` (2026-05-23) — terminology and resource-persistence note added_
_Part of: see `planning/features/particle_system_v2.md` for the full v2 overview_

## What

A global `ParticleQuality` setting scales particle counts across all effects, and a
`ParticleBudget` cap sheds low-priority ambient effects when the live count is too high.
A minimum of 1 particle per effect guarantees abilities always produce *something*
visible regardless of quality setting.

## Why

WASM web builds run on a very wide hardware range — from high-end desktops to mobile
browsers. Without quality scaling, the same counts that look good on a 3080 choke a
mobile iGPU. In high-activity encounters (raids, boss fights), uncapped particle
accumulation causes progressive frame drops. Both problems need a systemic solution,
not per-effect tweaks.

## Approach

### Quality multipliers

`ParticleQuality` resource with four levels:

| Level | Count multiplier |
|---|---|
| `Minimal` | 0.25× (minimum 1) |
| `Low` | 0.50× (minimum 1) |
| `Medium` | 0.75× |
| `High` | 1.0× (default) |

Applied at spawn time:
```rust
let actual_count = if let Some(q) = &layer.quality {
    match quality_res.level {
        Minimal => q.minimal,
        Low     => q.low,
        Medium  => q.medium,
        High    => layer.particle_count,
    }
} else {
    (layer.particle_count as f32 * quality_res.multiplier()).round().max(1.0) as u32
};
```

Per-layer quality overrides (optional — authoring explicit counts for each tier):
```ron
quality: ( minimal: 1, low: 2, medium: 4 ),   // high is always particle_count
```

### Effect priority

Per-effect `priority` field on `EffectDef`:
```ron
priority: Player,    // Player | Npc | Ambient   (default: Npc)
```

### Particle budget

`ParticleBudget` resource: `live_count: u32`, `max_count: u32` (default: 2000, configurable
in scene RON or engine config).

When `live_count + new_count > max_count`:
- `Ambient` priority effects: silently skip spawn
- `Npc` priority effects: halve count (min 1)
- `Player` priority effects: always spawn at full count; may briefly exceed budget

`live_count` increments on spawn, decrements on despawn (tracked in simulation tick).

### Quality action

```ron
Action::SetParticleQuality(Low)
```

Can be called from rules.ron on scene load or from a settings UI button.

**Files:**
- `capabilities/particle_budget.rs` (new, small)
- `schema/catalog.rs` — add `QualityOverride` struct + `quality`, `priority` fields to `LayerDef`
- `schema/actions.rs` — add `SetParticleQuality` variant

## Tasks

- [ ] Add `ParticleQuality` resource with `multiplier()` method
- [ ] Add `SetParticleQuality` to `Action` enum + executor dispatch
- [ ] Apply quality multiplier in `drain_particle_effects_system`
- [ ] Add `QualityOverride` struct + `quality` field to `LayerDef`
- [ ] Add `priority: EffectPriority` field to `EffectDef`
- [ ] Implement `ParticleBudget` resource with increment/decrement tracking
- [ ] Integrate budget gating into `drain_particle_effects_system`
- [ ] Decrement budget in particle despawn path
- [ ] Add quality setting to particles_demo (UI button or startup action)
- [ ] Tests:
  - [ ] `Minimal` quality → count scaled, never zero
  - [ ] Explicit `quality` overrides bypass multiplier
  - [ ] Budget cap: `Ambient` effect skipped; `Player` effect always fires
- [ ] Update `docs/20_data_formats.md`

## Open questions

- **`max_count` configuration**: per-scene in RON (`particle_budget: 2000`) or global
  engine constant? Per-scene is more flexible; a dense raid scene can set a higher cap
  than a calm exploration scene.
- **Budget and the pool renderer**: the pool renderer (shipped as feature 1) pre-allocates
  a fixed `Vec<PooledParticle>` (currently sized at runtime). The budget `max_count`
  should not exceed the pool capacity; add a `debug_assert` and document the constraint.
- **UI for quality setting**: in-game settings panel is not yet implemented. For now,
  expose via `SetParticleQuality` in rules.ron so a test scene can exercise it.
- **Resource persistence across scene transitions**: `ParticleQuality` must survive
  `Action::LoadScene` — it is a global resource, not a `LevelEntity`. Verify this is
  the case and add an integration test asserting the quality level does not reset when
  a new scene loads. This is explicitly required in the acceptance criteria above.

## Acceptance criteria

- `SetParticleQuality(Minimal)` reduces visible particles across all effects; every
  `SpawnEffect` still produces at least 1 particle
- With `ParticleBudget::max_count` set to 10, `Ambient` effects are dropped when full;
  a `Player` priority effect still fires
- Quality persists across scene transitions (resource is not reset on LoadScene)
- Setting quality to `High` at runtime restores full particle counts on next spawn

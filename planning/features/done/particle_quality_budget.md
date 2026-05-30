# Feature: Particle System v2 — 7. Quality Tiers & Particle Budget

_Status: Active_
_Planned at: `2cc61ca` (2026-05-19)_
_Reviewed at: `a16bd98` (2026-05-23) — terminology and resource-persistence note added_
_Verified at: `fa7d4bc` (2026-05-27) — open questions resolved, gap in From impl and budget tracking clarified_
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

`ParticleBudget` resource: `max_count: u32` (default: 2000, configurable per scene via
`particle_budget` field on `GameSceneV2`).

Live count is derived directly from the pool at spawn time — no separate tracking resource:
```rust
let live_count = pool.particles.iter().filter(|p| p.is_alive()).count() as u32;
```
This is always correct by construction. Particles die by time (`elapsed >= duration`), not
by explicit despawn, so increment/decrement tracking would require careful simulation-tick
integration for no benefit over a simple pool scan.

When `live_count + new_count > max_count`:
- `Ambient` priority effects: silently skip spawn
- `Npc` priority effects: halve count (min 1)
- `Player` priority effects: always spawn at full count; may briefly exceed budget

### Quality action

```ron
Action::SetParticleQuality(Low)
```

Can be called from rules.ron on scene load or from a settings UI button.

**Files:**
- `capabilities/particle_budget.rs` (new, small) — `ParticleQuality`, `ParticleBudget` resources
- `schema/catalog.rs` — add `QualityOverride` struct + `quality` field to `LayerDef`, `priority` field to `EffectDef`; also update `From<&EffectDef> for LayerDef` to copy `quality`
- `schema/actions.rs` — add `SetParticleQuality` variant

## Tasks

- [ ] Add `ParticleQuality` resource with `multiplier()` method
- [ ] Add `ParticleBudget` resource (`max_count: u32`, default 2000)
- [ ] Add `SetParticleQuality` to `Action` enum + executor dispatch
- [ ] Add `QualityOverride` struct + `quality` field to `LayerDef`; update `From<&EffectDef> for LayerDef` to copy `quality`
- [ ] Add `priority: EffectPriority` field to `EffectDef`
- [ ] Apply quality multiplier in `drain_particle_effects_system`
- [ ] Integrate budget gating into `drain_particle_effects_system` (derive live count from pool scan)
- [ ] Add `particle_budget` optional field to `GameSceneV2`; load it into `ParticleBudget` on scene load
- [ ] Add quality setting to particles_demo (startup action via rules.ron)
- [ ] Tests:
  - [ ] `Minimal` quality → count scaled, never zero
  - [ ] Explicit `quality` overrides bypass multiplier
  - [ ] Budget cap: `Ambient` effect skipped; `Player` effect always fires
  - [ ] Quality persists across scene transition (resource survives `LoadScene`)
- [ ] Update `docs/20_data_formats.md`

## Resolved decisions

- **`max_count` configuration**: per-scene RON field `particle_budget` on `GameSceneV2` (optional,
  default 2000). Loaded into `ParticleBudget` resource on each `SceneEvent::Ready`. Fits the
  data-driven philosophy; dense scenes can raise the cap without code changes.
- **Live count tracking**: derive from pool scan (`pool.particles.iter().filter(p.is_alive())`)
  at spawn time — not tracked via increment/decrement. Particles die by time, not despawn events.
- **`From<&EffectDef> for LayerDef` must be updated**: `quality` is per-layer; single-layer
  effects use `LayerDef::from(def)`, so the `From` impl must copy `quality`. Omitting this
  would silently disable per-layer quality overrides for single-layer effects.
- **`ParticleQuality` persistence**: plain `Resource` (not `LevelEntity`) — survives `LoadScene`
  by default. No special handling needed; verified against `action_executor.rs`.
- **UI for quality setting**: no in-game settings panel yet. Expose via `SetParticleQuality`
  action in `rules.ron` so the particles_demo can exercise it.

## Acceptance criteria

- `SetParticleQuality(Minimal)` reduces visible particles across all effects; every
  `SpawnEffect` still produces at least 1 particle
- With `ParticleBudget::max_count` set to 10, `Ambient` effects are dropped when full;
  a `Player` priority effect still fires
- Quality persists across scene transitions (resource is not reset on LoadScene)
- Setting quality to `High` at runtime restores full particle counts on next spawn

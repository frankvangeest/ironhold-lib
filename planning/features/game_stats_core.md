# Feature: Game Stats — Phase 1: Core Stat Model

_Status: Draft_
_Planned at: `fb97158` (2026-05-04) — hash updated after the 2026-09-03 `pkg/` history purge; the original citation, `1f63f4d`, was a pkg-only rebuild commit fully pruned during that purge, so this points to its parent instead (same code state)_

## What

Game designers can define named stats (health, mana, stamina, rage, hunger, …) in a `stats.ron` file using a simple RON schema. Each stat has a base value, min/max bounds, optional time-based regen, and threshold rules that fire events into the existing event pipeline when crossed. New `ModifyStat` and `SetStat` actions let rules.ron and state_machine.ron drive stat changes without any code.

## Why

Variables stored with `SetVariable` / `IncrementVariable` are plain strings — they have no bounds, no regen, and no way to react automatically when they hit a limit. A proper stat model eliminates the boilerplate of manually clamping values and checking thresholds in every rule, and gives game designers a declarative way to define character attributes that integrate cleanly with the existing event/action pipeline.

## Approach

### New RON file: `stats.ron`

Located at `assets/projects/{name}/stats.ron` (optional; engine skips gracefully if absent).

```ron
// stats.ron
(
    schema_version: 1,
    stats: {
        "health": (
            base: 100.0,
            min: 0.0,
            max: 100.0,
            regen_rate: 0.0,       // units per second; 0 = no regen
            regen_delay: 0.0,      // seconds after last decrease before regen starts
            thresholds: [
                ( when: BelowOrEqual(0.0),    emit: "stat.health.depleted" ),
                ( when: BelowPercent(0.25),   emit: "stat.health.low" ),
                ( when: AtOrAbovePercent(1.0),emit: "stat.health.full" ),
            ],
        ),
        "mana": (
            base: 50.0,
            min: 0.0,
            max: 50.0,
            regen_rate: 2.0,
            regen_delay: 3.0,
            thresholds: [
                ( when: AtOrAbovePercent(1.0), emit: "stat.mana.full" ),
            ],
        ),
    },
)
```

### Schema types (`schema/stats.rs`)

```rust
pub struct StatCatalog {
    pub schema_version: u32,
    pub stats: HashMap<String, StatDef>,
}

pub struct StatDef {
    pub base: f32,
    pub min: f32,
    pub max: f32,
    pub regen_rate: f32,        // default 0.0
    pub regen_delay: f32,       // default 0.0
    pub thresholds: Vec<StatThreshold>,
}

pub struct StatThreshold {
    pub when: ThresholdCondition,
    pub emit: String,           // event name fired into the event bus
}

pub enum ThresholdCondition {
    BelowOrEqual(f32),          // absolute value
    AboveOrEqual(f32),
    BelowPercent(f32),          // 0.0–1.0 fraction of max
    AtOrAbovePercent(f32),
}
```

### Runtime resource: `LoadedStats`

A Bevy resource holding live stat state, loaded from `stats.ron` at scene load time alongside the asset catalog:

```rust
pub struct LiveStat {
    pub def: StatDef,
    pub current: f32,
    pub regen_cooldown: f32,    // counts down to 0, then regen resumes
}

pub struct LoadedStats(pub HashMap<String, LiveStat>);
```

### New actions (`schema/actions.rs`)

```rust
ModifyStat { key: String, delta: f32 },   // clamps to [min, max]
SetStat    { key: String, value: f32 },   // clamps to [min, max]
```

RON usage:
```ron
do_actions: [ ModifyStat(key: "health", delta: -25.0) ]
do_actions: [ SetStat(key: "health", value: 100.0) ]
```

### Systems

- **`stat_loader_system`** — reads `stats.ron`, populates `LoadedStats` with `current = base` at scene load.
- **`stat_regen_system`** — runs each frame; ticks `regen_cooldown` down, then increments `current` by `regen_rate * delta_time`, clamped to max.
- **`stat_threshold_system`** — after any stat mutation, checks each threshold condition and emits a `GameEvent` if crossed (edge-triggered: only fires on the crossing frame, not every frame while below).
- **`action_executor`** — new arms for `ModifyStat` and `SetStat`; both clamp and trigger the threshold system.

### Integration with existing pipeline

Threshold events (`stat.health.depleted`, etc.) are emitted as `GameEvent` values — identical to events from collectibles or NPCs. `state_machine.ron` and `rules.ron` can react with any existing action without new code:

```ron
( on: "stat.health.depleted", to: "game_over" ),
( event: "stat.health.low", do_actions: [ PlaySound(key: "heartbeat") ] ),
```

### Entry point / project config

`ProjectConfig` (`{name}.project.ron`) gains an optional field:
```ron
stats: Some("stats/stats.ron"),
```
If absent, the stat system is inactive for that project.

## Tasks

- [ ] `schema/stats.rs` — `StatCatalog`, `StatDef`, `StatThreshold`, `ThresholdCondition`
- [ ] Register `StatCatalog` for RON deserialization; add `stats` field to `ProjectConfig`
- [ ] `LoadedStats` Bevy resource + `stat_loader_system`
- [ ] `stat_regen_system` (frame update, cooldown tracking)
- [ ] `stat_threshold_system` (edge-triggered event emission)
- [ ] `Action::ModifyStat` and `Action::SetStat` — schema + executor arms
- [ ] Integration test: threshold fires exactly once on crossing, not on every frame below
- [ ] Integration test: regen respects delay; does not exceed max
- [ ] Integration test: `ModifyStat` clamps to min/max
- [ ] RON validation test: `stats.ron` parses correctly
- [ ] Docs: update `20_data_formats.md` and `30_runtime_events_and_logic.md`
- [ ] Update `primitive_world` to use `ModifyStat` instead of `IncrementVariable("health", ...)`

## Open questions

- Should `stats.ron` be per-scene or per-project? Per-project seems right for most games (health persists across scenes), but some games want scene-local stats (a puzzle timer). Defer scene-local stats to a later iteration.
- Should `SetVariable` / `IncrementVariable` eventually be deprecated in favour of `SetStat` / `ModifyStat`? Probably yes, but not in Phase 1 — keep both for backwards compat.
- Where does stat state live during a scene transition? For now: stats persist in `LoadedStats` across scene loads (the resource is not despawned with the scene). Add save/restore later.

## Acceptance criteria

- Given a `stats.ron` with `health` at base 100, min 0, max 100, when `ModifyStat(key: "health", delta: -110.0)` is executed, then `current` is clamped to 0 and `stat.health.depleted` is emitted exactly once.
- Given a stat with `regen_rate: 5.0` and `regen_delay: 2.0`, when health is reduced, then regen does not begin until 2 seconds have elapsed, after which `current` climbs at 5 units/sec.
- Given a `state_machine.ron` with `( on: "stat.health.depleted", to: "game_over" )`, when health hits 0, then the state machine transitions to `game_over` without any code changes.
- Given a project with no `stats` field in `project.ron`, the engine loads and runs without error.

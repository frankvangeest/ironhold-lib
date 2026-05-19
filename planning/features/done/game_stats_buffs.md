# Feature: Game Stats — Phase 2: Buffs and Modifiers

_Status: Implemented_
_Planned at: `1f63f4d` (2026-05-04)_

_Depends on: `game_stats_core.md` (Phase 1) must be complete._

## What

Game designers can define named buff and debuff templates in RON and apply them to stats at runtime via new actions. A modifier stacks on top of a stat's base value, has an optional duration, and is removed automatically when it expires. Multiple modifiers on the same stat are resolved by a configurable stacking rule. Stats can optionally exceed their normal max (a `soft_max`) when boosted, then drain back to max once the buff expires.

## Why

Phase 1 covers the common case of flat `ModifyStat` deltas, but many game archetypes need temporary, reversible, and multiplicative effects: a speed potion that wears off, an armour debuff from a boss attack, an "overhealed" health buffer. Without a modifier stack these effects have to be tracked manually via variables and timer events, which is fragile and hard to author in RON.

## Approach

### Modifier definition in RON

Modifier templates live in `stats.ron` alongside stat definitions:

```ron
(
    schema_version: 1,
    stats: { /* … Phase 1 … */ },

    modifiers: {
        "speed_boost": (
            stat: "speed",
            kind: Multiplicative(1.5),   // ×1.5 speed
            duration_secs: Some(10.0),   // expires after 10 s; None = permanent
            stack_rule: Add,             // multiple instances add together
        ),
        "poison": (
            stat: "health",
            kind: Additive(-2.0),        // flat −2 per tick (see regen_rate interaction below)
            duration_secs: Some(8.0),
            stack_rule: Max,             // only the strongest poison applies
        ),
        "overheal": (
            stat: "health",
            kind: Additive(25.0),        // pushes above normal max via soft_max
            duration_secs: Some(15.0),
            stack_rule: Add,
        ),
    },
)
```

Modifier kinds:
- `Additive(f32)` — adds a flat delta to the effective value
- Multiplicative(f32)` — multiplies the base value (additive modifiers applied after)
- `Override(f32)` — forces the stat to a fixed value (takes priority over all other modifiers; last-applied wins)

Stack rules:
- `Add` — all active modifiers of the same key accumulate
- `Max` — only the modifier with the largest absolute magnitude applies
- `Replace` — each new application replaces the previous one

### Soft max

`StatDef` gains an optional `soft_max: Option<f32>`. When set, additive buffs can push `current` above `max` up to `soft_max`. Once the buff expires the value drains back to `max` at `regen_rate` (or immediately if `regen_rate` is 0).

```ron
"health": (
    base: 100.0, min: 0.0, max: 100.0, soft_max: Some(125.0),
    /* … */
),
```

### Runtime modifier state

Each active modifier instance is tracked separately:

```rust
pub struct ActiveModifier {
    pub key: String,            // references modifiers map
    pub remaining_secs: Option<f32>,
}
```

`LiveStat` gains `active_modifiers: Vec<ActiveModifier>`.

The effective value is computed each frame:
1. Start from `current` (the persistent value, already affected by `ModifyStat` / regen).
2. Sum all `Additive` modifiers (respecting stack rule).
3. Apply `Multiplicative` modifiers (respecting stack rule).
4. Apply `Override` if present.
5. Clamp to `[min, soft_max.unwrap_or(max)]`.

Thresholds evaluate the **effective** value, not the raw `current`.

### New actions

```rust
ApplyModifier { modifier_key: String },
RemoveModifier { modifier_key: String },  // removes all instances of that modifier
```

RON usage:
```ron
do_actions: [ ApplyModifier(modifier_key: "speed_boost") ]
do_actions: [ RemoveModifier(modifier_key: "poison") ]
```

### Modifier lifecycle events

When a modifier expires or is removed, the system emits an event:

```
stat.modifier.expired:speed_boost
stat.modifier.removed:poison
```

These feed back into `rules.ron` / `state_machine.ron` normally, allowing designers to chain effects (e.g., play a sound when a buff wears off).

### Systems

- **`stat_modifier_system`** — ticks `remaining_secs` each frame; emits expiry events; removes expired modifiers.
- **`stat_effective_value_system`** — recomputes effective value after any modifier change; feeds result to threshold system.
- **`action_executor`** — new arms for `ApplyModifier` and `RemoveModifier`.

### Order of operations (each frame)

1. `stat_modifier_system` — expire timed modifiers, emit expiry events
2. `stat_regen_system` — regen on raw `current`
3. `stat_effective_value_system` — recompute effective value from modifiers
4. `stat_threshold_system` — check thresholds against effective value

## Tasks

- [x] `StatDef`: add `soft_max: Option<f32>` field
- [x] `modifiers` map in `StatCatalog` schema
- [x] `ModifierDef`, `ModifierKind`, `StackRule` types in `schema/stats.rs`
- [x] `ActiveModifier` + `active_modifiers` field on `LiveStat`
- [x] Effective value computation function (additive → multiplicative → override → clamp)
- [x] `stat_modifier_system` — tick durations, remove expired, emit lifecycle events
- [x] `stat_effective_value_system`
- [x] `Action::ApplyModifier` and `Action::RemoveModifier` — schema + executor
- [x] Integration test: additive stacking accumulates correctly
- [x] Integration test: `Max` stack rule ignores weaker modifier
- [ ] Integration test: timed modifier expires and effective value returns to base _(headless Bevy time simulation; deferred)_
- [x] Integration test: soft_max allows overheal; drains to max after buff expires
- [x] Integration test: threshold evaluates effective value, not raw `current`
- [x] RON validation: modifier template round-trips through serde (7 new tests)
- [ ] Docs: extend `20_data_formats.md` and `30_runtime_events_and_logic.md`

## Open questions

- **Poison as a modifier vs regen**: A poison with `Additive(-2.0)` and a duration makes semantic sense, but it currently affects the *static* effective value, not a per-tick drain. Should poison be modelled as a negative `regen_rate` modifier instead? A negative regen modifier is cleaner for DoT effects; additive is better for flat penalties (−armour). Revisit during implementation.
- **Save/load**: Should active modifier state (remaining durations) be serialised? Likely yes for games with save files, but out of scope for Phase 2.
- **Per-entity stats**: Phase 1 and 2 treat stats as a single global pool (the player). Multi-entity stats (each NPC has its own health) requires stats to be ECS components, not a resource. Leave for a later phase.

## Acceptance criteria

- Given a `speed_boost` modifier with `Multiplicative(1.5)` and `duration_secs: 10.0`, when `ApplyModifier(modifier_key: "speed_boost")` is executed, then `effective_speed = base_speed × 1.5` for 10 seconds, after which it returns to `base_speed` and `stat.modifier.expired:speed_boost` is emitted.
- Given two `poison` modifiers (stack_rule: Max) applied simultaneously, then only the stronger one affects the effective value.
- Given `health` with `soft_max: 125.0`, when an `overheal` buff of `+25` is applied, then `effective_health` reaches 125 and the existing `stat.health.full` threshold does not incorrectly fire until the value crosses 100%.
- Given a threshold `BelowPercent(0.25)` that evaluates effective value, when a debuff pushes effective health below 25% while raw current stays above, then the threshold fires.

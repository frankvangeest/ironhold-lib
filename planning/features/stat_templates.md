# Feature: Stat Templates — Per-Prefab Instance Stats

_Status: Draft_
_Planned at: `270ff7e` (2026-05-05)_

## What

A designer writing a goblin guard prefab should be able to declare its stat shape once — base HP,
min/max, regen, thresholds — and have the engine automatically create an independent `LiveStat`
for every spawned instance. With 20 goblins on the map, `stats.ron` stays empty of goblin entries
and the state machine stays empty of per-goblin rules. The goblin's own `.behavior.ron` handles
everything about its own life and death, using `{self}` substitution to self-reference.

## Why

The current Phase 1 model requires one `stats.ron` entry per live entity. For two static goblins
that is manageable; for a wave of 20 dynamically-spawned goblins it becomes unmaintainable — 20
catalog entries, 20 threshold handlers, 20 death rules in the state machine. Dynamic spawning makes
it completely unworkable because spawn IDs are assigned at runtime, not authoring time.

This feature unblocks:
- Dynamic enemy waves (`Action::Spawn` with health automatically initialised from prefab template)
- Reusable enemy prefabs with no per-instance catalog boilerplate
- Cleaner state machines with no per-instance rules (death, low-HP logic lives in behavior files)
- Rollback networking and P2P: instance stat state co-located with entity state (see below)

---

## Architecture decision: Component vs LoadedStats

This is the most consequential design choice. Both options were evaluated in full; the discussion
is preserved here because it directly informs the hybrid model that was chosen.

### Option A — Keep stats in `LoadedStats` (global resource, keyed by string)

**How it works:** `LoadedStats(HashMap<String, LiveStat>)` remains the single runtime store.
Instance stats are created with key `"{spawn_id}.{stat_name}"` (e.g. `"goblin_01.health"`)
at spawn time and removed at despawn. All existing executor code works unchanged — the key is longer.

**Pros:**
- Zero breaking changes to executor, threshold system, or `SceneStateParams`.
- RON actions address stats by plain string key, consistent with `GameVariables`.
- Cross-entity stat reads are free: `ModifyStat(key: "goblin_01.health", ...)` works from any rule.
- One mental model for designers: all keyed game data lives in resources.
- Despawn cleanup is trivial: drain all keys prefixed `"{spawn_id}."` — one line.

**Cons:**
- Not ECS-native: no change detection, no parallel scheduling, no `With<>` filter.
- `stat_threshold_system` is a single-threaded scan of every live stat every frame.
- Rust's `HashMap` uses a randomised seed — iteration order is non-deterministic across
  runs and machines. This **breaks deterministic replay** (Beta 0.5). Fixable with `IndexMap`
  but requires deliberate preparation before Beta 0.5.
- For snapshot/restore (Beta 0.5), `LoadedStats` must be registered as `ReflectResource`
  separately from entity state — two snapshot targets instead of one.
- For **rollback networking** (P2P, fighting games, `bevy_ggrs`), the framework snapshots
  entity component state automatically. Resources require custom per-resource snapshot logic.
  `LoadedStats` as a resource means writing bespoke rollback plumbing for every stat, forever.
- Orphan risk: if a stat key is not cleaned up on despawn the entry lingers silently.

### Option B — `StatMap` as a Bevy Component on the entity

**How it works:** A `StatMap(IndexMap<String, LiveStat>)` component is inserted on prefab entities
at spawn time. `stat_threshold_system` queries `Query<(&SpawnId, &mut StatMap)>`.

**Pros:**
- ECS-native: Bevy's parallel executor and change detection apply.
- Automatic cleanup: despawning the entity removes the component with no explicit stat cleanup.
- `IndexMap` gives deterministic iteration order — replay determinism solved structurally.
- `bevy_ggrs` (rollback networking) snapshots entity component state automatically.
  With a `Reflect` derive, `StatMap` is snapshot-ready with no custom rollback code.
- For fighting games and P2P: each entity carries its own stat state; snapshot/restore is
  the ECS world — one target, handled by the framework.
- Bevy inspector and `DynamicScene` serialisation see entity stats as part of entity state.

**Cons:**
- `ModifyStat` executor arm needs a `Query<(&SpawnId, &mut StatMap)>` to find and mutate
  entity stats by spawn ID, in addition to the existing `LoadedStats` lookup for global stats.
  This means two code paths, selected by whether the key contains a dot.
- `SceneStateParams` gains one more field (the stat map query). Within the composed
  `SystemParam` budget — does not count against the outer 16-param limit.
- Multiple stats per entity require `StatMap` to be an inner `IndexMap`, which is the same
  data structure as `LoadedStats` — just co-located with the entity. Small additional indirection.

---

### Verdict: Hybrid — `StatMap` component for instance stats, `LoadedStats` for global stats

**The decision was revised after evaluating the full roadmap, including P2P networking,
fighting game support, and rollback (Beta 0.6+).**

#### Why the hybrid model, not pure Option B

Global stats (player health, game-wide mana) are singletons with no owning entity — they belong in
a resource. The player entity doesn't always exist (menu scenes), and `player_health` is addressed
by plain key in existing RON with no entity prefix. Moving it to a component would break backward
compatibility with no benefit. The hybrid keeps the correct home for each kind of stat:

| Stat kind | Storage | Why |
|---|---|---|
| Instance stats (enemy HP, boss rage) | `StatMap` component on entity | Is entity state; must be rolled back with the entity |
| Global stats (player health, game-wide) | `LoadedStats` resource | Singleton; no owning entity; addressed by plain key |

`ModifyStat` routes based on the key: a dot signals `"{spawn_id}.{stat_name}"` → entity lookup;
no dot → `LoadedStats` lookup. The designer-facing RON API (`ModifyStat(key: "{self}.health",
delta: -35.0)`) is identical either way.

#### Why P2P and fighting games tip the scale toward components

1. **Rollback networking is the standard for fighting games and P2P.** `bevy_ggrs` (the canonical
   Bevy rollback solution) snapshots entity component data automatically. It does not automatically
   handle `Resource` rollback — you write custom save/restore logic per resource. With `StatMap` as
   a component, deriving `Reflect` is enough; `bevy_ggrs` handles the rest. With `LoadedStats`,
   every rollback frame requires bespoke serialisation of the entire HashMap — custom code that
   must stay correct as the game grows.

2. **Migration cost grows with content.** Every behavior file, rule, and scene authored against
   a resource-keyed stat API makes a future migration heavier. The stat template feature is
   already touching spawn-time init, the executor, and the threshold system. Doing it now costs
   little extra; doing it after content accumulates costs significantly more.

3. **The designer API does not change.** `ModifyStat(key: "{self}.health", delta: -35.0)` works
   identically whether the implementation routes to a HashMap or a component. The dot convention
   absorbs the implementation change invisibly.

4. **`IndexMap` gives deterministic iteration by construction.** Using `IndexMap<String, LiveStat>`
   in `StatMap` gives deterministic iteration order (insertion order) without requiring a separate
   `IndexMap` migration before Beta 0.5. The replay determinism concern is resolved structurally.

5. **Automatic cleanup is correct behaviour.** Entity stats genuinely belong to the entity.
   Having them disappear when the entity despawns is not just convenient — it is the right model.
   The orphan risk of the resource approach disappears entirely.

#### Obligation for global stats (`LoadedStats`)

`LoadedStats` remains for global stats. Before Beta 0.5 it must:
- Switch internal `HashMap` to `IndexMap` (deterministic iteration for replay)
- Derive `Reflect` and register as `ReflectResource` (snapshot/restore)

Same obligation applies to `GameVariables`. These are Beta 0.5 pre-conditions, not part of
this feature.

---

## Proposed approach

### 1. Schema: `stat_templates` field on `PrefabDef`

```ron
// prefabs/prefabs.ron
"npc_goblin_guard": (
  kind: "primitive",
  ...
  stat_templates: [
    (
      key: "health",
      base: 60.0,
      min: 0.0,
      max: 60.0,
      regen_rate: 0.0,
      regen_delay: 0.0,
      thresholds: [
        ( when: BelowOrEqual(0.0), emit: "stat.{self}.health.depleted" ),
      ],
    ),
  ],
),
```

`{self}` in `emit` strings is substituted with the entity's spawn ID at spawn time. A goblin
spawned as `goblin_01` emits `"stat.goblin_01.health.depleted"`; one as `wave_3_goblin_07`
emits `"stat.wave_3_goblin_07.health.depleted"`. The field is a `Vec` so a prefab can
carry multiple stats (health + stamina + rage, etc.).

```rust
// schema/stats.rs
#[derive(Deserialize, Debug, Clone)]
pub struct StatTemplateDef {
    pub key: String,   // stat name within this entity; StatMap key
    pub base: f32,
    pub min: f32,
    pub max: f32,
    #[serde(default)] pub regen_rate: f32,
    #[serde(default)] pub regen_delay: f32,
    #[serde(default)] pub thresholds: Vec<StatThreshold>,
}
```

`StatThreshold` is the same type already used in `StatDef` — no new schema type needed.
`StatTemplateDef` and `StatDef` are structurally identical minus the catalog wrapper;
they can be unified into one type if desired (see Open questions).

### 2. `StatMap` component

```rust
// schema/stats.rs
#[derive(Component, Reflect, Default, Clone)]
pub struct StatMap(pub IndexMap<String, LiveStat>);
```

`IndexMap` (from the `indexmap` crate) preserves insertion order, giving deterministic iteration.
`Reflect` is derived immediately so `bevy_ggrs` and `DynamicScene` can handle it without further
work when networking is implemented. `Clone` is required for Bevy's rollback snapshot machinery.

### 3. Spawn-time `StatMap` initialisation

In both prefab spawn paths in `scene_loader.rs` (composite and single-mesh) and in
`entity_spawner.rs` / `drain_spawn_queue_system` (for `Action::Spawn`):

```rust
if !prefab.stat_templates.is_empty() {
    let mut stat_map = StatMap::default();
    for tpl in &prefab.stat_templates {
        let def = StatDef {
            base: tpl.base, min: tpl.min, max: tpl.max,
            regen_rate: tpl.regen_rate, regen_delay: tpl.regen_delay,
            thresholds: tpl.thresholds.iter().map(|t| StatThreshold {
                when: t.when.clone(),
                emit: t.emit.replace("{self}", spawn_id),
            }).collect(),
        };
        stat_map.0.insert(tpl.key.clone(), LiveStat::new(def));
    }
    commands.entity(entity).insert(stat_map);
}
```

No `LoadedStats` writes for instance stats. Despawn cleanup is automatic.

### 4. Routing in `ModifyStat` / `SetStat` executor arms

The key convention doubles as a routing signal. A dot distinguishes instance stats from globals:

```rust
Action::ModifyStat { key, delta } => {
    if let Some((entity_id, stat_name)) = key.split_once('.') {
        // Instance stat — find entity, mutate StatMap component.
        if let Some(&e) = spawn_registry.entities.get(entity_id) {
            if let Ok(mut stat_map) = scene_state.stat_map_query.get_mut(e) {
                if let Some(stat) = stat_map.0.get_mut(stat_name) {
                    stat.apply_delta(delta);
                }
            }
        }
    } else {
        // Global stat — LoadedStats resource (unchanged).
        if let Some(stat) = scene_state.loaded_stats.0.get_mut(&key) {
            stat.apply_delta(delta);
        }
    }
}
```

`scene_state.stat_map_query: Query<&mut StatMap>` is added to `SceneStateParams`. Because
`SceneStateParams` is a composed `SystemParam`, this adds one field to the struct without
incrementing the outer system's param count.

### 5. `stat_threshold_system` — two branches

```rust
pub fn stat_threshold_system(
    mut stat_map_query: Query<&mut StatMap>,
    mut loaded_stats: ResMut<LoadedStats>,
    mut game_events: MessageWriter<GameEvent>,
) {
    // Instance stats: iterate all entities carrying a StatMap.
    for mut stat_map in stat_map_query.iter_mut() {
        for stat in stat_map.0.values_mut() {
            fire_threshold_crossings(stat, &mut game_events);
        }
    }
    // Global stats: iterate LoadedStats resource (unchanged).
    for stat in loaded_stats.0.values_mut() {
        fire_threshold_crossings(stat, &mut game_events);
    }
}
```

`fire_threshold_crossings` is a private helper extracted from the existing single-loop body.

### 6. `{self}` substitution extended to action key fields

The entity FSM interpreter already substitutes `{self}` in event patterns and target strings
(`Despawn("{self}")`, `EmitEvent("...:{self}")`). Extend it to the `key` field of `ModifyStat`
and `SetStat` so behavior files can write:

```ron
ModifyStat(key: "{self}.health", delta: -35.0)
```

resolved to `ModifyStat(key: "goblin_01.health", delta: -35.0)` at dispatch time.

### 7. Entity behavior file — complete goblin lifecycle

With templates and component stats, the goblin guard's full lifecycle moves into its own
behavior file. `state_machine.ron` needs zero goblin-specific rules:

```ron
// assets/projects/primitive_world/behaviors/goblin_guard.behavior.ron
(
  schema_version: 1,
  initial_state: "alive",
  states: [
    (
      name: "alive",
      entry_actions: [],
      exit_actions: [],
      on: [
        // Player attacks this goblin (F key within radius).
        ( event: "entity.interacted:{self}", do_actions: [
            PlaySound(key: "goblin_hit"),
            ModifyStat(key: "{self}.health", delta: -35.0),
        ]),
        // Goblin contacts the player.
        ( event: "npc.player_reached:{self}", do_actions: [
            PlaySound(key: "goblin_growl"),
            PlaySound(key: "player_pain"),
            IncrementVariable("score", -10),
            IncrementVariable("health", -30),
            ModifyStat(key: "player_health", delta: -30.0),
        ]),
        // This goblin's health hits zero — threshold fires from StatMap.
        ( event: "stat.{self}.health.depleted", do_actions: [
            Despawn("{self}"),
            PlaySound(key: "collect_coin"),
            IncrementVariable("score", 50),
        ]),
      ],
    ),
  ],
  transitions: [],
)
```

### 8. Before / after comparison

**Before (current, 2 goblins):**
- `stats.ron`: 3 entries (player_health, goblin_01_health, goblin_02_health)
- `state_machine.ron` playing state: 8 goblin-specific rules
- Adding a 3rd goblin: +1 stats.ron entry, +4 state_machine.ron rules, +code if dynamic

**After (with this feature):**
- `stats.ron`: 1 entry (player_health only)
- `state_machine.ron` playing state: 0 goblin-specific rules
- `behaviors/goblin_guard.behavior.ron`: 3 rules, shared by every instance
- Adding a 3rd goblin (static): one entity in the scene — nothing else
- Adding a 3rd goblin (dynamic, `Action::Spawn`): one Spawn action — nothing else

---

## Tasks

### Schema
- [ ] Add `stat_templates: Vec<StatTemplateDef>` to `PrefabDef` in `schema/catalog.rs`
- [ ] Add `StatTemplateDef` to `schema/stats.rs` (or unify fields with `StatDef`)
- [ ] Add `StatMap` component to `schema/stats.rs` with `Reflect`, `Clone`, `Default`
- [ ] Register `StatMap` with `app.register_type::<StatMap>()` in `lib.rs`

### Spawn-time initialisation
- [ ] Composite prefab path in `scene_loader.rs`: insert `StatMap` if `stat_templates` non-empty
- [ ] Single-mesh prefab path in `scene_loader.rs`: same
- [ ] `entity_spawner.rs` / `drain_spawn_queue_system`: same for `Action::Spawn`

### Executor
- [ ] `ModifyStat` arm: dot-routing to `StatMap` query or `LoadedStats`
- [ ] `SetStat` arm: same routing
- [ ] Add `stat_map_query: Query<&mut StatMap>` field to `SceneStateParams`
- [ ] Remove `Despawn` prefix-drain (no longer needed — component auto-cleanup)

### Threshold system
- [ ] Extract `fire_threshold_crossings` helper from existing loop body
- [ ] Add `Query<&mut StatMap>` branch to `stat_threshold_system`

### `{self}` substitution
- [ ] Extend entity FSM interpreter to substitute `{self}` in `ModifyStat.key` and `SetStat.key`

### primitive_world wiring
- [ ] Add `stat_templates` to `npc_goblin_guard` prefab; remove goblin entries from `stats.ron`
- [ ] Author `assets/projects/primitive_world/behaviors/goblin_guard.behavior.ron`
- [ ] Add `behavior: "behaviors/goblin_guard.behavior.ron"` to goblin prefab def
- [ ] Remove goblin-specific rules from `state_machine.ron` playing state
- [ ] Remove `SetStat` goblin reset calls from `state_machine.ron` playing entry_actions

### Tests
- [ ] RON validation: `stat_templates` field parses on `PrefabDef`
- [ ] RON validation: `StatTemplateDef` with `{self}` in emit parses correctly
- [ ] Integration: spawn entity with template → `StatMap` component exists with correct values
- [ ] Integration: `ModifyStat` with dot key mutates correct entity's `StatMap`
- [ ] Integration: `ModifyStat` without dot key still mutates `LoadedStats`
- [ ] Integration: threshold crossing on `StatMap` fires correct `GameEvent::Trigger`
- [ ] Integration: despawn entity → `StatMap` gone (no explicit cleanup needed)

### Docs
- [ ] `docs/20_data_formats.md`: `stat_templates` field, `StatMap` component, key routing convention
- [ ] `docs/30_runtime_events_and_logic.md`: behavior file + stat template authoring pattern

---

## Open questions

- **`StatTemplateDef` vs `StatDef` unification:** They are structurally identical. Consider a
  single `StatDef` type used both in `StatCatalog` (global stats) and `stat_templates` (instance
  stats), eliminating duplication. The `key` field in `StatTemplateDef` maps to the catalog key
  in `StatCatalog`; the threshold `emit` field may contain `{self}` only in templates, not in
  catalog entries. A shared type with a note in the doc comment is the simplest path.

- **`stat_regen_system` coverage:** Currently iterates `LoadedStats`. After this feature it also
  needs to iterate `StatMap` components. Same two-branch pattern as `stat_threshold_system`.

- **Player health as a component (future):** `player_health` stays in `LoadedStats` for now.
  If the player entity is always present during gameplay, moving it to a `StatMap` on the player
  entity is a clean future step — it removes the hybrid routing for the most important stat and
  makes the player entity fully self-contained for rollback purposes. Plan this separately.

- **`bevy_ggrs` integration surface:** When rollback networking is implemented, `StatMap` needs
  to be added to the GGRS snapshot schedule. `Reflect` + `Clone` are already required by this
  feature; the integration itself (adding `StatMap` to the rollback type registry) is a one-liner
  at that point.

---

## Acceptance criteria

- Given a prefab with `stat_templates: [(key: "health", base: 60.0, ...)]` and two scene entities
  `goblin_01` and `goblin_02` using that prefab, when the scene loads, both entities have a
  `StatMap` component with `stat_map.0["health"].current == 60.0`. `LoadedStats` contains no
  goblin entries.

- Given a goblin entity with `StatMap["health"].current == 60.0`, when
  `ModifyStat(key: "goblin_01.health", delta: -35.0)` executes, then
  `StatMap["health"].current == 25.0` on that entity.

- Given a goblin entity with `StatMap["health"].current == 0.0`, when `stat_threshold_system`
  runs, then `GameEvent::Trigger("stat.goblin_01.health.depleted")` is emitted exactly once.

- Given `Despawn("goblin_01")` executes, the goblin entity is gone and its `StatMap` component
  is gone with it. No explicit cleanup step is required; no `LoadedStats` keys are affected.

- Given a goblin behavior file with `ModifyStat(key: "{self}.health", delta: -35.0)` and an
  entity with spawn ID `goblin_02`, when `entity.interacted:goblin_02` fires, then
  `goblin_02`'s `StatMap["health"]` is decremented by 35.

- Given `ModifyStat(key: "player_health", delta: -30.0)` (no dot), then `LoadedStats["player_health"]`
  is decremented — `StatMap` on any entity is unaffected.

- Given the full primitive_world refactor, when two goblin guards are present and a third is added
  to the scene, no changes to `stats.ron`, `state_machine.ron`, or any Rust file are required.

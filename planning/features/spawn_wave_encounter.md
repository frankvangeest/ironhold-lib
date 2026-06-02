# Feature: Spawn Wave / Encounter System

_Status: Draft_
_Planned at: `4c47cc6` (2026-06-02)_

---

> ## Pre-implementation checklist
>
> - [ ] **Decide: wave catalog location.** Options: (a) `waves: HashMap<String, WaveDef>` block in `assets.ron` (same pattern as `effects`); (b) a separate `encounters.ron` per project. Recommendation: **`assets.ron`** — waves are reusable assets, not scene-specific; one file to register, consistent with effects and models.
>
> - [ ] **Decide: wave completion detection.** `wave.complete:{id}` should fire when all entities spawned by the wave are dead. Options: (a) track spawn IDs in `ActiveWaves` and poll `SpawnRegistry` each frame; (b) listen for a `Despawn` event broadcast; (c) require designers to wire a `StatMap` death event. Recommendation: **poll `SpawnRegistry`** — it's the authoritative source of live entities; no coupling to cause-of-death; works regardless of whether the entity died by combat, fell out of world, or was manually despawned.
>
> - [ ] **Decide: inter-step delay semantics.** `delay_secs` on `WaveStepDef` — does it mean "delay before this step starts after the previous step fired" or "delay before this step starts after all entities of the previous step are dead"? Recommendation: **delay from previous step fire time** (time-based, not kill-based). This gives designers predictable timings. If kill-based gating is needed, designers can wire a `wave.step_complete:{id}:{step}` event and a separate rule — but that's the designer's responsibility, not the default.
>
> - [ ] **Decide: spawn IDs for wave entities.** The wave system spawns entities using `Action::Spawn`. Each spawned entity needs a stable ID so the wave system can track it in `SpawnRegistry`. Recommendation: auto-generate wave-scoped IDs: `{wave_id}_step{N}_{M}` (e.g. `patrol_wave_step0_0`, `patrol_wave_step0_1`). Designers can override with `id_prefix` on `WaveStepDef` to get predictable IDs for behavior wiring.
>
> - [ ] **Decide: looping `wave.complete` semantics.** When `looping: true`, does `wave.complete:{id}` fire at the end of each loop, or only when the wave is `StopWave`d? Recommendation: **fire at end of each loop iteration** (each time all entities from one pass are dead). This lets designers react to each wave clear (reward, score, difficulty increase) before the next loop begins.

---

## What

A data-driven system for spawning timed sequences of enemies or props. Designers define **waves** in `assets.ron` — ordered lists of spawn steps with delays and optional positions. A single `StartWave("wave_id")` action fires the whole encounter; the system handles timing, entity tracking, and emits `wave.complete:{id}` when all spawned entities are dead.

Replaces manual chains of `Spawn` + `EmitEventAfterDelay` + counting in `GameVariables` that currently require many rules to achieve a simple encounter.

---

## Why

Without this, a three-wave encounter in `rules.ron` requires:
- One `Spawn` per enemy (times 3 waves × N enemies = many rules)
- Manual `EmitEventAfterDelay` to trigger the next wave
- Variable tracking to count alive enemies
- No loop support

With this feature, the same encounter is one `WaveDef` block and two rules (`StartWave` on trigger, react to `wave.complete`).

---

## Schema

### `AssetCatalog` — new `waves` map (`schema/catalog.rs`)

```ron
// assets.ron
(
    schema_version: 3,
    models: { ... },
    waves: {
        "patrol_wave": (
            steps: [
                ( prefab: "orc_scout", count: 2, delay_secs: 0.0 ),
                ( prefab: "orc_archer", count: 3, delay_secs: 5.0 ),
                ( prefab: "orc_chief", count: 1, delay_secs: 8.0, id_prefix: Some("chief") ),
            ],
            looping: false,
        ),
        "arena_loop": (
            steps: [
                ( prefab: "arena_goblin", count: 5, delay_secs: 0.0,
                  positions: [(-3.0,0.0,0.0),(0.0,0.0,-3.0),(3.0,0.0,0.0),(0.0,0.0,3.0),(-1.5,0.0,1.5)] ),
            ],
            looping: true,
            loop_delay_secs: 3.0,
        ),
    },
)
```

```rust
// schema/catalog.rs — in AssetCatalog
#[serde(default)]
pub waves: HashMap<String, WaveDef>,
```

### New `WaveDef` + `WaveStepDef` (`schema/catalog.rs`)

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct WaveDef {
    /// Ordered spawn steps. Each step fires `delay_secs` after the previous step fired.
    pub steps: Vec<WaveStepDef>,

    /// When true, the wave restarts after all spawned entities are dead.
    #[serde(default)]
    pub looping: bool,

    /// Seconds to wait between loop iterations (after all entities die, before restart).
    /// Ignored when `looping: false`. Default: 0.0.
    #[serde(default)]
    pub loop_delay_secs: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct WaveStepDef {
    /// Prefab key to spawn (must exist in PrefabCatalog).
    pub prefab: String,

    /// Number of entities to spawn in this step. Default: 1.
    #[serde(default = "default_wave_count")]
    pub count: u32,

    /// Seconds after the previous step fired before this step fires. Default: 0.0.
    #[serde(default)]
    pub delay_secs: f32,

    /// Optional fixed world positions for spawned entities.
    /// If count > positions.len(), positions cycle. If empty, uses `spawn_point` or origin.
    #[serde(default)]
    pub positions: Vec<(f32, f32, f32)>,

    /// Named spawn point from the scene's `spawn_points` map. Used when `positions` is empty.
    #[serde(default)]
    pub spawn_point: Option<String>,

    /// Optional prefix for spawn IDs. Default: `"{wave_id}_step{N}_{M}"`.
    /// Set to `Some("boss")` to get predictable IDs like `"boss_0"` for behavior wiring.
    #[serde(default)]
    pub id_prefix: Option<String>,
}
```

### New actions (`schema/actions.rs`)

```ron
StartWave("patrol_wave")     // begin executing a named wave
StopWave("patrol_wave")      // halt a running wave; pending steps cancelled; no wave.complete fired
```

```rust
// schema/actions.rs
/// Begin executing a named wave defined in AssetCatalog.waves.
/// No-op if the wave is already running (logs a warning).
StartWave(String),
/// Halt a running wave. Any pending steps are cancelled.
/// Does NOT emit wave.complete — use to abort encounters.
/// No-op if the wave is not running.
StopWave(String),
```

### New pipeline events

```ron
wave.started:{id}          // wave execution began
wave.step.fired:{id}:{n}   // step N spawned its entities (n = 0-based step index)
wave.complete:{id}         // all spawned entities are dead (fires each loop iteration when looping)
wave.loop:{id}:{n}         // loop iteration n began (n = 1-based; fires after loop_delay_secs)
```

---

## Runtime

### `ActiveWaves` resource (`capabilities/wave_spawner.rs`)

```rust
#[derive(Resource, Default)]
pub struct ActiveWaves {
    pub waves: HashMap<String, ActiveWave>,
}

pub struct ActiveWave {
    pub def: WaveDef,
    pub current_step: usize,
    pub step_timer: f32,          // seconds until next step fires
    pub phase: WavePhase,
    pub spawned_ids: Vec<String>, // all spawn IDs ever created by this wave run
    pub loop_count: u32,
}

pub enum WavePhase {
    Running,
    WaitingForKills,  // all steps fired; waiting for all entities to die
    LoopDelay(f32),   // looping wave between iterations; f32 = seconds remaining
}
```

### `wave_tick_system` (`capabilities/wave_spawner.rs`)

Runs in `Update`. For each `ActiveWave`:

1. **Running phase**: decrement `step_timer` by `delta_secs`. When ≤ 0:
   - Fire the current step: push `Action::Spawn` onto the queue for each entity (position from `positions` list cycling, or `spawn_point`, or origin). Record spawn IDs in `spawned_ids`. Emit `wave.step.fired:{id}:{n}`.
   - Advance `current_step`. If more steps remain, set `step_timer` to next step's `delay_secs`.
   - If all steps fired, transition to `WaitingForKills`.

2. **WaitingForKills phase**: check `SpawnRegistry` — are any `spawned_ids` still present? If none remain:
   - Emit `wave.complete:{id}` (or `wave.loop:{id}:{n+1}` when looping).
   - If `looping: false` → remove from `ActiveWaves`.
   - If `looping: true` → transition to `LoopDelay(def.loop_delay_secs)`, reset `spawned_ids`, reset `current_step = 0`, increment `loop_count`.

3. **LoopDelay phase**: decrement timer. When ≤ 0 → transition to `Running`, set `step_timer` to first step's `delay_secs`.

### `StartWave` executor arm (`action_executor.rs`)

```rust
Action::StartWave(key) => {
    if let Some(wave_def) = loaded_catalog.waves.get(&key) {
        if active_waves.waves.contains_key(&key) {
            warn!("StartWave: wave '{}' already running", key);
        } else {
            active_waves.waves.insert(key.clone(), ActiveWave {
                def: wave_def.clone(),
                current_step: 0,
                step_timer: wave_def.steps.first().map_or(0.0, |s| s.delay_secs),
                phase: WavePhase::Running,
                spawned_ids: vec![],
                loop_count: 0,
            });
            game_events.write(GameEvent::Trigger(format!("wave.started:{}", key)));
        }
    } else {
        warn!("StartWave: no wave '{}' in catalog", key);
    }
}
```

---

## Worked example

```ron
// assets.ron
waves: {
    "dungeon_encounter": (
        steps: [
            ( prefab: "skeleton", count: 3, delay_secs: 0.0 ),
            ( prefab: "skeleton_archer", count: 2, delay_secs: 4.0 ),
            ( prefab: "skeleton_king", count: 1, delay_secs: 10.0, id_prefix: Some("king") ),
        ],
        looping: false,
    ),
}

// logic/rules.ron
( on: "entity.entered:dungeon_trigger", do_actions: [StartWave("dungeon_encounter")] ),
( on: "wave.complete:dungeon_encounter", do_actions: [
    EmitEvent("dungeon.cleared"),
    PlaySound(key: "fanfare"),
    SetEntityVisible(entity: "dungeon_door", visible: true),
] ),
```

---

## New Rust changes

- `schema/catalog.rs` — add `waves: HashMap<String, WaveDef>`, `WaveDef`, `WaveStepDef`; update `AssetCatalog::validate()` to check prefab key existence.
- `schema/actions.rs` — add `StartWave(String)`, `StopWave(String)`.
- `capabilities/wave_spawner.rs` (new file) — `ActiveWaves`, `ActiveWave`, `WavePhase`, `wave_tick_system`.
- `capabilities/mod.rs` — register module + system.
- `runtime/scene_manager/action_executor.rs` — handle `StartWave`, `StopWave`.
- `runtime/scene_manager/mod.rs` — clear `ActiveWaves` on `Action::LoadScene`.

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `WaveDef` + `WaveStepDef` in `schema/catalog.rs`; `waves` map in `AssetCatalog`
- [ ] `AssetCatalog::validate()` cross-checks wave prefab keys against `PrefabCatalog`
- [ ] `StartWave(String)` + `StopWave(String)` actions
- [ ] `ActiveWaves` resource + `ActiveWave` / `WavePhase` structs
- [ ] `wave_tick_system` — step timer, spawn dispatch, kill detection, loop logic
- [ ] Executor arms for `StartWave` / `StopWave`
- [ ] `ActiveWaves` cleared on `LoadScene`
- [ ] Pipeline events: `wave.started`, `wave.step.fired`, `wave.complete`, `wave.loop`
- [ ] Demo: add a wave to `primitive_world` or `3rd_person_game_demo`; wire `wave.complete` to unlock a door
- [ ] Integration tests: step delay timing, completion detection, loop restart, `StopWave` cancels, invalid wave key logs warning
- [ ] Docs: `WaveDef`, `StartWave`, `StopWave`, `wave.*` events in `docs/20_data_formats.md` + `docs/30_runtime_events_and_logic.md`

---

## Open questions

- **Kill-based step gating**: some games wait for all step-N enemies to die before spawning step N+1. This is "kill-gated waves" vs "time-gated waves". v1 is time-gated. Kill-gated requires a mode flag on `WaveStepDef` (`wait_for_kills: bool`); deferred to a follow-up pass if designers request it.
- **Wave state in behavior files**: can a per-entity behavior file check whether a wave is running? Not in v1 — behavior files work via events, not resource inspection. A designer can wire `wave.started:{id}` → `EmitEvent` to a state that behavior files can react to.
- **Concurrent waves**: multiple `StartWave` actions on different wave IDs are fully independent. All run simultaneously. The only shared resource is `ActiveWaves` (HashMap keyed by wave ID).
- **Spawn queue interaction**: wave spawns go through `Action::Spawn` → `PendingEntitySpawns` → `drain_spawn_queue_system` (max 2 per frame). A step spawning 10 entities at once takes 5 frames to fully spawn. The wave system should account for this: step N is "fired" when the spawns are enqueued, not when all entities appear. This is acceptable for v1.

---

## Acceptance criteria

- Given `StartWave("patrol_wave")` fires, step 0 entities spawn immediately; step 1 entities spawn after `delay_secs`; `wave.started:patrol_wave` is emitted.
- Given all spawned wave entities are despawned, `wave.complete:patrol_wave` is emitted.
- Given `looping: true`, after all entities die and `loop_delay_secs` elapses, the wave restarts and `wave.loop:patrol_wave:1` is emitted.
- Given `StopWave("patrol_wave")`, pending steps are cancelled; `wave.complete` is NOT emitted; already-spawned entities remain.
- Given `StartWave` with an unknown key, a warning is logged and no wave starts.
- Given a wave with `positions`, entities spawn at those positions cycling; entity 0 at position 0, entity 1 at position 1, entity 5 (in a 5-position list, count 6) at position 0 again.
- Given a scene transition (`LoadScene`), all `ActiveWaves` are cleared.

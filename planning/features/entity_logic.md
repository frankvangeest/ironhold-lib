# Feature: Entity Logic (FSM v1)

_Status: Draft_
_Planned at: `e957e9f` (2026-04-29)_

## What

Lets game authors attach a data-driven finite-state machine to any entity in a scene. Behavior is
authored in a `.behavior.ron` file (same schema as `state_machine.ron`) and referenced from the
entity's prefab definition. The entity's FSM reacts to events scoped to it — trigger zone
enter/exit and player interaction — and fires entity-targeted actions (play animation, despawn
self, emit an event) that are resolved by the executor without any recompilation.

Two new sensor capabilities complete the picture: **TriggerZone** (Rapier sensor that emits
`entity.entered:{id}` / `entity.exited:{id}`) and **Interactable** (proximity + action-key check
that emits `entity.interacted:{id}`).

## Why

All current interactive behavior (collectibles, NPC AI, score) is hardwired in Rust. Adding a
door that opens on interaction, or an NPC with a simple patrol-to-idle transition, requires
writing a new capability and recompiling. This feature makes per-entity behavior fully
data-configurable and unblocks any game that needs interactive world objects.

## Approach

### `.behavior.ron` files

Reuse `StateMachineAsset` verbatim — no new schema type needed. Behavior files live in
`assets/projects/{name}/behaviors/` by convention. They are loaded as `Handle<StateMachineAsset>`.

```ron
// behaviors/door.behavior.ron
(
  schema_version: 1,
  initial_state: "closed",
  global_on: [],
  states: [
    ( name: "closed",
      entry_actions: [ PlayAnimationOn("close") ],
      exit_actions: [],
      on: [] ),
    ( name: "open",
      entry_actions: [ PlayAnimationOn("open") ],
      exit_actions: [],
      on: [] ),
  ],
  transitions: [
    ( from: Some("closed"), on: "entity.interacted:{self}", to: "open" ),
    ( from: Some("open"),   on: "entity.interacted:{self}", to: "closed" ),
  ],
)
```

### `{self}` placeholder

In a behavior file, `{self}` in any event pattern or action target string is substituted at
runtime with the entity's spawn ID. This makes behavior files reusable across multiple entities
without modification. The substitution happens inside `entity_fsm_interpreter_system` before
matching, not at load time.

### New components

| Component | Purpose |
|---|---|
| `PendingBehavior(Handle<StateMachineAsset>)` | Loaded first frame; replaced once asset resolves |
| `BehaviorHandle(Handle<StateMachineAsset>)` | Stable handle kept alive to prevent asset eviction |
| `EntityFsmState { current: String }` | The entity's current named FSM state; mutable by the interpreter |

Follow the same resolve-on-load pattern as `PendingAnimationPolicy` → `AnimationPolicyComponent`.

### Prefab authoring

```ron
// prefabs/prefabs.ron
PrefabDef(
  id: "door",
  model: "models/door.glb",
  behavior: Some("behaviors/door.behavior.ron"),
  interactable: Some(InteractableDef( radius: 2.0 )),
  // ...
)
```

Both `behavior` and `interactable` are `Option` fields; omitting them changes nothing for
existing prefabs.

### `entity_fsm_interpreter_system`

A new system that runs in `Update` alongside `fsm_interpreter_system`. It:

1. Reads `GameEvent`, `UiEvent`, and `SceneEvent` (same sources as the global interpreter).
2. For each entity with `(BehaviorHandle, EntityFsmState, SpawnId)`:
   - Substitutes `{self}` → entity's spawn ID in every transition `on` pattern.
   - Checks whether any read event matches a transition or `on` binding.
   - On match: queues exit actions → queues entry actions → updates `EntityFsmState::current`.
3. Entity actions carrying `"self"` as a target are rewritten to the entity's concrete spawn ID
   before being pushed to the global `ActionQueue`.

The system must consume events *after* they have been written (standard `MessageReader` behaviour)
so it shares the same frame semantics as the global interpreter.

### Entity-targeted action variants

Only the minimum set needed for the two examples. All carry a `target: String` which may be
`"self"` inside behavior files (rewritten by the interpreter before queuing).

| Action | Executor behaviour |
|---|---|
| `PlayAnimationOn { target: String, clip: String }` | Finds entity by spawn ID, sends animation clip |
| `DespawnSelf` | No explicit target needed; executor finds entity via context (see below) |
| `EmitEvent(String)` | Emits `GameEvent::Trigger` with the (already-substituted) string |

`DespawnSelf` is special: the interpreter tags the queued action with the entity's spawn ID
internally. The simplest implementation is to translate `DespawnSelf` to
`Action::Despawn(spawn_id)` at queue-push time inside the interpreter — the executor already
handles `Despawn`.

### TriggerZone capability

New `TriggerZone` component on a prefab. Backed by a Rapier sensor collider (same approach as
`Collectable`). On player enter/exit the sensor emits:

```
GameEvent::Trigger("entity.entered:{spawn_id}")
GameEvent::Trigger("entity.exited:{spawn_id}")
```

No actions are taken directly — responses are wired in RON.

### Interactable capability

New `Interactable { radius: f32 }` component. Each frame, checks distance between the player
entity and the interactable entity. When the player is within `radius` and presses the interact
key (default `"F"`, configurable via key bindings as `"interact"`), emits:

```
GameEvent::Trigger("entity.interacted:{spawn_id}")
```

A proximity indicator label (optional `hint_text: Option<String>` on `InteractableDef`) can be
shown/hidden via `Visibility` when the player enters/exits range.

## Tasks

### Schema
- [ ] Add `behavior: Option<String>` to `PrefabDef`
- [ ] Add `interactable: Option<InteractableDef>` to `PrefabDef`
- [ ] Add `trigger_zone: Option<TriggerZoneDef>` to `PrefabDef`
- [ ] Add `Action::PlayAnimationOn { target: String, clip: String }` to `schema/actions.rs`
- [ ] Add `Action::EmitEvent(String)` to `schema/actions.rs`
- [ ] (`DespawnSelf` translated to `Despawn` by interpreter — no new Action variant needed)

### Runtime — component loading
- [ ] `PendingBehavior`, `BehaviorHandle`, `EntityFsmState` components in `scene_manager/mod.rs`
- [ ] `resolve_pending_behaviors_system` — polls `PendingBehavior` handles, inserts
  `BehaviorHandle` + `EntityFsmState` once loaded (mirrors `resolve_animation_policy_system`)
- [ ] Wire `resolve_pending_behaviors_system` into `lib.rs` `Update` schedule

### Runtime — entity spawning
- [ ] `entity_spawner`: when spawning a prefab with `behavior` set, insert `PendingBehavior`
- [ ] `entity_spawner`: when `interactable` set, insert `Interactable`; when `trigger_zone` set,
  insert `TriggerZone` + Rapier sensor collider

### Interpreter
- [ ] `entity_fsm_interpreter_system` in `runtime/scene_manager/message_interpreter.rs`
- [ ] `{self}` substitution helper (used by both event matching and action rewriting)
- [ ] Wire into `lib.rs` alongside `fsm_interpreter_system`

### Action executor
- [ ] `Action::PlayAnimationOn` arm in `action_executor_system` — find entity by spawn ID,
  send animation
- [ ] `Action::EmitEvent` arm — write `GameEvent::Trigger`

### Capabilities
- [ ] `capabilities/trigger_zone.rs` — Rapier sensor, enter/exit detection, emit `GameEvent`
- [ ] `capabilities/interactable.rs` — proximity check + key binding, emit `GameEvent`, optional hint label
- [ ] Register both in `lib.rs`

### Examples
- [ ] New project `entity_logic_demo` — two scenes: door (open/close on interact) + NPC (idle →
  wander → interact)
- [ ] Or add a "behavior demo" scene to `primitive_world` if a new project feels too heavy

### Validation & tests
- [ ] `ron_validation` tests for `behaviors/` folder (valid + invalid schema)
- [ ] Integration test: entity FSM transitions on `GameEvent` (closed → open → closed)
- [ ] Integration test: `{self}` substitution routes event to correct entity, not others
- [ ] Integration test: `TriggerZone` emits correct event names
- [ ] Integration test: `DespawnSelf` translated to `Despawn(id)` and entity removed

### Docs
- [ ] `docs/30_runtime_events_and_logic.md` — entity FSM section, `{self}` substitution, new events
- [ ] `docs/STATUS.md` — mark 0.4 done, add new actions/events to ABI table
- [ ] `crates/ironhold_core/src/CLAUDE.md` — note entity FSM rules (parallel to global FSM rules)
- [ ] `CLAUDE.md` — mention `behaviors/` folder in project layout block

## Open questions

- **`PlayAnimationOn` executor**: the animation system currently targets the player entity via a
  dedicated resource. Entity animation may need a more general "find entity by spawn ID, send
  `AnimationTransitionEvent`" path. Investigate before implementing.
- **Interaction key configurability**: should `"interact"` default to `"F"` hardcoded, or should
  `InteractableDef` carry `key: Option<String>` so different interactables can use different keys?
  Probably default `"F"` + per-entity override is cleanest.
- **Hint label visibility**: the proximity hint label design may overlap with `WorldLabel`.
  Consider whether `Interactable` just emits a `GameEvent` for show/hide (and RON wires the
  response) rather than managing visibility directly — that keeps the capability dumb.

## Acceptance criteria

- Given a prefab with `behavior: Some("behaviors/door.behavior.ron")` and
  `interactable: Some(InteractableDef(radius: 2.0))`, when the player walks within 2 m and
  presses the interact key, the door entity transitions from `"closed"` to `"open"` and the
  `"open"` animation plays on the door (not the player).
- Given a second door entity with the same behavior file but a different spawn ID, interacting
  with one door does not affect the other.
- Given a `TriggerZone` prefab, when the player enters the collider,
  `GameEvent::Trigger("entity.entered:{id}")` is emitted and any RON rule matching that event
  fires its actions.
- All new integration tests pass; all existing tests continue to pass.

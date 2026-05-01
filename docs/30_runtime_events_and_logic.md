# Runtime: Events and Logic

> **Doc type:** Design Doc (vision)
>
> **Status legend:**
> - ✅ **Implemented** — exists in code today
> - 🧪 **Prototype / Partial** — exists but incomplete or unstable
> - 🧭 **Planned** — intended design; not implemented yet

## Status
🧭 Planned (with a growing ✅ implemented subset)

## Goal
Provide a single, cross-platform logic model that is **driven by data** and does not require recompilation for most gameplay iteration. The runtime should be consistent between native and web/WASM.

## Design overview
Ironhold’s runtime model is built around three layers:

1. **Messages (events)** 🧭  
   Standardized, engine-level messages representing “something happened” (input, UI, triggers, scene lifecycle, animation markers).
2. **Actions** 🧭  
   Discrete, explicit commands representing “do something” (load scene, quit, play animation, spawn entity, set variable, etc.).
3. **Execution** 🧭  
   A controlled executor that applies actions in a predictable order, enabling testing, determinism strategies, and clear debugging.

### Why separate messages and actions?
- Messages are **observations** (facts): a button was pressed, an input action fired, a trigger entered.
- Actions are **commands** (intent): load a scene, quit the game, play a sound, set a state variable.

This separation helps:
- keep data-defined logic declarative and testable 🧭
- support deterministic simulation by controlling when/how actions apply 🧭
- decouple UI/input from gameplay logic 🧭

## Implementation snapshot (today)
This section is factual and reflects what exists right now.

- ✅ A robust action layer exists: `ActionQueue` plus actions such as `LoadScene(String)`, `Quit`, `Log`, `Spawn`, `PlayAnimation`, `PlaySound`, `PlayMusicLoop`, `StopMusic`, `SetVolume`, `Preload`, `EnterState`, `SetVariable`, `IncrementVariable`, and more.
- ✅ UI events exist (`UiEvent`) and are emitted by UI button interaction and key bindings; button `action` strings have the `"ui."` prefix stripped before firing (e.g. `action: "ui.dance"` → `UiEvent::ButtonPressed("dance")`).
- ✅ Gameplay events exist (`GameEvent::Trigger(String)`) and are emitted by capabilities (physics sensors, etc.); the trigger name is used as-is in the rules pipeline.
- ✅ Input messages (`InputActionMessage`) decouple raw input from gameplay logic (point-to-point, not through the pipeline).
- ✅ Scene lifecycle events (`SceneEvent`) are emitted during loading transitions.
- ✅ A message interpreter maps UI events, game events, and scene events to actions using data-defined rules loaded from `logic/rules.ron`.
- ✅ Rules support an optional `when` guard: rules with `when: Some("state_name")` only fire while the interpreter is in that named state; rules with `when: None` (or omitted) fire in any state.
- ✅ A **FSM interpreter** maps events to actions and state transitions using a `StateMachineAsset` loaded from `logic/state_machine.ron`. Replaces `rules.ron` for FSM projects. States declare `entry_actions`, `exit_actions`, and in-state `on` bindings; `transitions` drive state changes; `global_on` fires from any state.
- ✅ An **entity FSM interpreter** (`entity_fsm_interpreter_system`) runs the same `StateMachineAsset` format per entity. Entities with a `behavior` path on their `PrefabDef` load an independent FSM; `{self}` in event patterns and action targets is substituted with the entity's spawn ID at runtime, making behavior files reusable across instances. See [Entity FSM section](#entity-fsm-beta-04) below.
- ✅ An action executor applies actions; notably:
  - `LoadScene(path)` loads a scene asset and transitions to `LoadingScene`.
  - `LoadSceneOverlay(path)` / `UnloadOverlay` load/unload overlay scenes (e.g. pause menu).
  - `Quit` requests app exit (writes `AppExit::Success`).
  - `Spawn { prefab, id }` / `Despawn(id)` spawn/remove prefab instances by ID.
  - `PlayAnimation(clip)` plays an animation on available controllers.
  - `PlayAnimationOn { target, clip }` plays an animation on a specific entity by spawn ID.
  - `EmitEvent(name)` emits a `GameEvent::Trigger`; `{self}` is substituted in behavior contexts.
  - `PlaySound(key)` / `PlayMusicLoop(key)` / `StopMusic` control audio.
  - `SetVolume(pct)` sets global volume (0–100).
  - `Preload(path)` warms the asset cache for a scene before it is needed.
  - `EnterState(name)` transitions the interpreter to a named logic state.
  - `Log(msg)` emits an `info!` log line.
- ✅ `LogicState` resource tracks the current named state (default `""`). Rules with a matching `when` guard become active; others are suppressed. FSM transitions update it directly in the interpreter.
- ✅ `DebugState` resource exposes `last_action`, `app_state`, `scene`, `frame`, `logic_state`, and `score` for observability and browser-based testing.

## Event model (planned)
We standardize runtime messages so content can bind to them consistently.

### Core message categories

#### 1) InputAction ✅
Abstract input actions (not raw keys/buttons):
- `input.move` (vector2)
- `input.look` (vector2)
- `input.jump` (pressed/released)

**Why:** input mappings vary by platform/device, but gameplay should consume stable action names.

#### 2) UiEvent 🧭
UI interactions and higher-level UI events:
- `ui.button_pressed` (id)
- `ui.quit_requested`
- `ui.menu_opened` (name)

**Why:** keep UI wiring declarative; bind UI events to gameplay actions.

#### 3) SceneEvent ✅
Scene lifecycle events fire in this order:

| Event name | When it fires | Use for |
|-----------|---------------|---------|
| `scene.requested:<stem>` | Load has been requested (asset not yet read) | Show a loading indicator |
| `scene.loaded:<stem>` | RON asset deserialized; entities **not yet spawned** | Pre-spawn setup (e.g. set state, queue audio) |
| `scene.ready:<stem>` | All entities spawned and ready | Start gameplay logic, trigger transitions |
| `scene.unloading:<stem>` | Before a full scene replace (not overlays) | Teardown (e.g. stop music, save state) |

`<stem>` is the filename without `.scene.ron` (e.g. `"main"` for `scenes/main.scene.ron`).

**Why:** data-defined flows (menus → loading → gameplay) need stable hooks at each stage.

#### 4) GameEvent / Trigger ✅
Physics sensors and gameplay capabilities emit named triggers via `GameEvent::Trigger(String)`.
The name is used as-is in the rules pipeline — the caller is responsible for namespacing:
- `"player.jumped"` — emitted by `CharacterController` on every successful jump ✅
- `"entity.collected:<id>"` — collectible sensor overlap ✅
- `"entity.entered:<id>"` — trigger zone entry (Rapier sensor; `FixedUpdate`) ✅
- `"entity.exited:<id>"` — trigger zone exit (Rapier sensor; `FixedUpdate`) ✅
- `"entity.interacted:<id>"` — player within radius + pressed F ✅
- `"npc.player_spotted:<id>"` — NPC entered alert state after detecting player ✅
- `"npc.player_reached:<id>"` — NPC reached the player's position ✅
- `"npc.player_lost:<id>"` — NPC lost sight of player and returned to idle ✅
- `"collision.hit:<id>"` — impact event 🧭

**Why:** drive scripted logic without bespoke code; keeps capabilities decoupled from the rules they trigger.

#### 5) AnimationMarker 🧭
Animation timeline markers:
- `anim.marker` (entity, marker_name)

**Why:** enable animation-driven gameplay events without code changes.

### Event naming rules (planned)
- Events are referenced by **strings** (e.g. `"ui.start"`, `"scene.loaded"`). 🧭
- Names should be **stable** and **namespaced**.
- Payload schemas should be documented and versioned.

## Action model
Actions represent explicit operations the runtime can execute.

### Action categories

#### Scene actions 🧭
- `LoadScene(path)` ✅
- `UnloadScene(name)` 🧭

#### App/system actions ✅
- `Quit` ✅
- `Log(message)` ✅

#### Entity actions
- `Spawn(asset_path)` ✅
- `Despawn(entity)` 🧭
- `SetTransform(entity, transform)` 🧭

#### Animation/audio actions
- `PlayAnimation(clip_id)` ✅ — plays a named animation by semantic ID (see AnimationPolicy)
- `PlaySound(audio_key)` ✅ — plays a sound by `AssetCatalog` audio key; fire-and-forget (entity despawns on completion); warns and no-ops for unsupported formats (`.wav`, `.ogg`, `.mp3` supported) or missing catalog keys

#### State/variables actions
- `EnterState(name)` ✅ — transitions the interpreter to a named logic state; rules with a matching `when` guard become active, others are suppressed; empty string returns to stateless (always-fire) default
- `SetVariable(key, value)` ✅ — writes a named string value into `GameVariables`; readable by data-bound UI labels; `DebugState.score` is derived from the `"score"` key
- `IncrementVariable(key, delta)` ✅ — parses the variable as `i32` and adds the delta; missing or unparseable values default to `0`

#### UI actions 🧭
- `ShowUi(panel_id)`
- `HideUi(panel_id)`

### Action semantics (planned)
- Actions are executed in a defined order per tick/frame.
- Actions should be **idempotent** where reasonable.
- Action execution should be observable for debugging and replay.

## Logic rules: mapping Events → Actions (planned)
The heart of data-driven behavior is a rule system that maps incoming messages to actions.

### Rule concepts
- **Bindings**: “When event X happens, run actions Y.” 🧭
- **Filters/conditions**: restrict rules (by entity tags, state variables, scene, etc.). 🧭
- **Parameters**: allow payload data to flow into actions (e.g., button id → scene path). 🧭

### Example ✅
```ron
// logic/rules.ron
(
    schema_version: 2,
    rules: [
        // No `when` guard — fires in any state
        ( on: "scene.ready:main", do_actions: [ EnterState("playing"), PlayMusicLoop("bg_music") ] ),

        // `when` guard — only fires while in the named state
        ( on: "ui.button_pressed:start_game", when: Some("menu"), do_actions: [ Log("Starting"), LoadScene("scenes/main.scene.ron") ] ),
        ( on: "ui.button_pressed:quit",       when: Some("menu"), do_actions: [ Quit ] ),

        ( on: "ui.button_pressed:toggle_pause", when: Some("playing"), do_actions: [ LoadSceneOverlay("scenes/pause.scene.ron"), EnterState("paused") ] ),
        ( on: "ui.button_pressed:toggle_pause", when: Some("paused"),  do_actions: [ UnloadOverlay, EnterState("playing") ] ),
    ],
)
```

Event name format: `"<domain>.<type>:<payload>"`. The interpreter matches the full string against each rule's `on` field. UI button events are always `"ui.button_pressed:<trigger>"` where the trigger is the button's `action` field with the `"ui."` prefix stripped.

The optional `when` field (type `Option<String>`, `#[serde(default)]`) gates a rule to a named logic state. Omitting it is equivalent to `when: None`, which fires in every state.

## Execution model (planned)

### Interpreter 🧭
Transforms messages into actions using the rule set for the current scene/project.

### Executor 🧭
Applies actions to the world. Key design points:
- The executor is the **single place** where side effects happen.
- The executor can be made deterministic/fixed-step later.

### Ordering & determinism notes 🧭
- For determinism, prefer a **fixed tick** for gameplay actions.
- Separate deterministic gameplay actions from non-deterministic presentation effects.

(See `docs/40_determinism_and_networking.md` for design notes.)

## Milestone mapping (suggested)

- **Milestone 0.1 + 0.2 (implemented)** ✅
  - ✅ `UiEvent::ButtonPressed` emitted by UI buttons; mapped to actions via data-defined rules
  - ✅ Full action set: `LoadScene`, `Quit`, `Log`, `Spawn`, `PlayAnimation`, `PlaySound`, `SetVariable`, `IncrementVariable`
  - ✅ `InputAction` abstraction (`Move`, `Turn`, `Look`, `Jump`, `Run`)
  - ✅ Scene lifecycle events: `SceneEvent::{Requested, Loaded, Ready, Unloading}`
  - ✅ Data-defined `logic/rules.ron` and `logic/state_machine.ron` wired to interpreter + executor
  - ✅ `DebugState` resource for runtime observability (`frame`, `app_state`, `last_action`, `scene`, `logic_state`, `score`)
  - ✅ `GameEvent::Trigger(String)` for physics sensors and gameplay capabilities

- **Milestone: Rule bindings v2** 🧭
  - Conditions/filters on rules (entity tags, state variables, scene)
  - Parameter flow from event payload into actions

- **Milestone: Trigger / Collision events** 🟡
  - `GameEvent::Trigger` and collectible sensors implemented ✅
  - Zone enter/exit events (`zone.entered:<id>`) 🧭

- **Milestone: Deterministic core hooks** 🧭
  - Fixed tick loop for gameplay
  - Replayable input stream

## Non-goals (for now)
- Fully deterministic rendering/audio 🧭
- A complete visual scripting system 🧭

## Appendix: Implemented subset (today)

### Messages ✅
- `UiEvent::ButtonPressed(String)` — emitted by UI buttons and key bindings; flows into the pipeline as `"ui.button_pressed:{trigger}"`
- `GameEvent::Trigger(String)` — emitted by gameplay capabilities (physics sensors, etc.); name is used as-is in the pipeline (e.g. `"entity.collected:coin_01"`)
- `SceneEvent::{Requested(String), Loaded(String), Ready(String), Unloading(String)}` — scene lifecycle
- `InputAction::{Move(Vec2), Turn(f32), Look(Vec2), Jump(bool), Run(bool)}` — abstract input (point-to-point, not pipeline)
- `InputActionMessage { entity: Entity, action: InputAction }` — input bound to a specific entity (point-to-point, not pipeline)

### Actions ✅
- `LoadScene(String)` — loads a `.scene.ron` and transitions to `LoadingScene`
- `LoadSceneOverlay(String)` — loads a `.scene.ron` as an overlay (e.g. pause menu)
- `UnloadOverlay` — despawns all overlay entities
- `ToggleOverlay(String)` — opens overlay if none is active, closes if one is
- `Quit` — writes `AppExit::Success`
- `Log(String)` — emits an `info!` log line
- `Spawn { prefab, id }` — spawns a prefab instance by key; auto-generates ID if omitted
- `Despawn(String)` — removes a previously spawned entity by its spawn ID
- `PlayAnimation(String)` — plays an animation by semantic ID (see AnimationPolicy)
- `PlaySound(String)` — fire-and-forget audio by catalog key; warns for unsupported formats or missing keys
- `PlayMusicLoop(String)` — starts a looping background music track by catalog key
- `StopMusic` — stops the current background music
- `SetVolume(u32)` — sets global audio volume 0–100
- `Preload(String)` — warms the asset cache for a `.scene.ron` before it is needed
- `EnterState(String)` — transitions the interpreter to a named logic state; `""` returns to stateless default
- `SetVariable(String, String)` — writes a named string value into `GameVariables`; readable by data-bound UI labels; `DebugState.score` is derived from the `"score"` key
- `IncrementVariable(String, i32)` — parses the variable as `i32` and adds the delta; missing or unparseable values default to `0`

### Infrastructure ✅
- `ActionQueue` — FIFO queue processed each frame by `action_executor_system` (push order equals execution order)
- `LogicState` resource — tracks the current named state (default `""`); checked by both interpreters
- `DebugState` resource — tracks `frame`, `app_state`, `last_action`, `scene`, `logic_state`; serialised to DOM on WASM for browser testing
- Data-defined rules loaded from `logic/rules.ron` via `LogicRulesAsset`; rules support optional `when: Option<String>` state guard
- `StateMachineAsset` loaded from `logic/state_machine.ron` via `fsm_interpreter_system`; used when `state_machine_path` is set in the project config

### FSM asset schema (`logic/state_machine.ron`) ✅

```ron
(
    schema_version: 1,
    initial_state: "menu",   // sets LogicState on load; no entry actions fired at startup

    global_on: [
        // Fires from any state; does not change state.
        ( event: "ui.button_pressed:debug_reload", do_actions: [ Log("reload") ] ),
    ],

    states: [
        (
            name: "playing",
            entry_actions: [ PlayMusicLoop("bg_music") ],   // queued when entering this state
            exit_actions:  [ StopMusic ],                    // queued when leaving this state
            on: [
                // In-state bindings: fire while in "playing", do not change state.
                ( event: "ui.button_pressed:dance", do_actions: [ PlayAnimation("dance") ] ),
            ],
        ),
        (
            name: "paused",
            entry_actions: [ LoadSceneOverlay("scenes/pause.scene.ron") ],
            exit_actions:  [ UnloadOverlay ],
            on: [],
        ),
    ],

    transitions: [
        // Omit `from` to match any current state.
        ( on: "scene.ready:main", to: "playing" ),

        // Explicit from/to.
        ( from: Some("playing"), on: "ui.button_pressed:toggle_pause", to: "paused" ),
        ( from: Some("paused"),  on: "ui.button_pressed:toggle_pause", to: "playing" ),
    ],
)
```

**Execution order per transition:** exit actions → state change → entry actions.
The engine handles this automatically; authors do not write `EnterState` in FSM data.

> New Messages or Actions must update `docs/STATUS.md` (Engine ABI section), this appendix, and `docs/20_data_formats.md` with an authoring example.

---

## Entity FSM (Beta 0.4)

### Overview

Each entity can run its own independent `StateMachineAsset` loaded from a `.behavior.ron` file. The behavior file uses the same format as `logic/state_machine.ron` (states, transitions, entry/exit actions, global_on). The key difference is the `{self}` placeholder, which is substituted with the entity's spawn ID at runtime.

### Authoring

Add `behavior` (and optionally `interactable` or `trigger_zone`) to a `PrefabDef`:

```ron
// prefabs/prefabs.ron
"collectible_box": (
  kind: "primitive",
  model: "Cuboid",
  behavior: Some("behaviors/collectible_box.behavior.ron"),
  interactable: Some(( radius: 2.5 )),
  primitive: Some(( size: Some((0.8, 0.8, 0.8)), color: Some((0.9, 0.7, 0.2)) )),
),
```

The behavior file uses `{self}` as a placeholder for the entity's spawn ID:

```ron
// behaviors/collectible_box.behavior.ron
(
  schema_version: 1,
  initial_state: "idle",
  global_on: [],
  states: [
    ( name: "idle",      entry_actions: [],                                   exit_actions: [], on: [] ),
    ( name: "collected", entry_actions: [ PlaySound("score"), Despawn("{self}") ], exit_actions: [], on: [] ),
  ],
  transitions: [
    ( from: Some("idle"), on: "entity.interacted:{self}", to: "collected" ),
  ],
)
```

When two boxes `box_01` and `box_02` share this file, interacting with `box_01` fires `entity.interacted:box_01`, which only matches `box_01`'s transition (pattern `entity.interacted:{self}` → `entity.interacted:box_01`). `box_02` is not affected.

### `{self}` substitution rules

`{self}` is replaced with the entity's spawn ID in:
- Transition `on` patterns
- In-state and `global_on` event patterns
- `Despawn("{self}")` → `Despawn("box_01")`
- `PlayAnimationOn { target: "{self}", clip: "open" }` → `target: "box_01"`
- `EmitEvent("door.opened:{self}")` → `"door.opened:box_01"`
- `Spawn { prefab: "...", id: Some("{self}_debris") }` → id `"box_01_debris"`

### New capabilities

| Capability | PrefabDef field | Emitted event | Notes |
|---|---|---|---|
| `CharacterController` | `components.movement` | `player.jumped` | Emitted on every jump; bind sound/effect in `state_machine.ron` |
| `TriggerZone` | `trigger_zone: Some(( radius: 2.0 ))` | `entity.entered:{id}` / `entity.exited:{id}` | Rapier sphere sensor; runs in `FixedUpdate` |
| `Interactable` | `interactable: Some(( radius: 2.5 ))` | `entity.interacted:{id}` | Player within radius + press F; runs in `Update` |

### System ordering

The interpreter chain in `Update` is:

```
interactable_system  →  message_interpreter_system
                     →  fsm_interpreter_system
                     →  entity_fsm_interpreter_system
                     →  action_executor_system
```

`trigger_zone_system` runs in `FixedUpdate` alongside `collectible_system`, so its events are visible to all three interpreter systems in the following `Update` tick.


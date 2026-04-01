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

- ✅ A robust action layer exists: `ActionQueue` plus actions such as `LoadScene(String)`, `Quit`, `Log`, `Spawn`, `PlayAnimation`, `PlaySound`, `PlayMusicLoop`, `StopMusic`, `SetVolume`, `Preload`, `EnterState`, and more.
- ✅ UI messages exist (`UiMessage`) and are emitted by UI button interaction; button `action` strings have the `"ui."` prefix stripped before firing (e.g. `action: "ui.dance"` → `UiMessage::ButtonPressed("dance")`).
- ✅ Input messages (`InputActionMessage`) decouple raw input from gameplay logic.
- ✅ Scene lifecycle events (`SceneEvent`) are emitted during loading transitions.
- ✅ A message interpreter maps UI messages and scene events to actions using data-defined rules loaded from `logic/rules.ron`.
- ✅ Rules support an optional `when` guard: rules with `when: Some("state_name")` only fire while the interpreter is in that named state; rules with `when: None` (or omitted) fire in any state.
- ✅ An action executor applies actions; notably:
  - `LoadScene(path)` loads a scene asset and transitions to `LoadingScene`.
  - `LoadSceneOverlay(path)` / `UnloadOverlay` load/unload overlay scenes (e.g. pause menu).
  - `Quit` requests app exit (writes `AppExit::Success`).
  - `Spawn { prefab, id }` / `Despawn(id)` spawn/remove prefab instances by ID.
  - `PlayAnimation(clip)` plays an animation on available controllers.
  - `PlaySound(key)` / `PlayMusicLoop(key)` / `StopMusic` control audio.
  - `SetVolume(pct)` sets global volume (0–100).
  - `Preload(path)` warms the asset cache for a scene before it is needed.
  - `EnterState(name)` transitions the interpreter to a named logic state.
  - `Log(msg)` emits an `info!` log line.
- ✅ `LogicState` resource tracks the current named state (default `""`). Rules with a matching `when` guard become active; others are suppressed.
- ✅ `DebugState` resource exposes `last_action`, `app_state`, `scene`, `frame`, and `logic_state` for observability and browser-based testing.

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
Scene lifecycle:
- `scene.requested` (path/name)
- `scene.loaded`
- `scene.ready`

**Why:** data-defined flows (menus → loading → gameplay) need stable hooks.

#### 4) Trigger / Collision 🧭
Spatial interactions:
- `trigger.enter` (entity_a, entity_b, trigger_id)
- `trigger.exit` (…)
- `collision.hit` (…)

**Why:** drive scripted logic without bespoke code.

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
- `SetVar(key, value)` 🧭
- `IncVar(key, delta)` 🧭

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
  - ✅ `UiMessage::ButtonPressed` emitted by UI buttons; mapped to actions via data-defined rules
  - ✅ Full action set: `LoadScene`, `Quit`, `Log`, `Spawn`, `PlayAnimation`, `PlaySound`
  - ✅ `InputAction` abstraction (`Move`, `Turn`, `Look`, `Jump`, `Run`)
  - ✅ Scene lifecycle events: `SceneEvent::{Requested, Loaded, Ready}`
  - ✅ Data-defined `logic/rules.ron` wired to interpreter + executor
  - ✅ `DebugState` resource for runtime observability

- **Milestone: Rule bindings v2** 🧭
  - Conditions/filters on rules (entity tags, state variables, scene)
  - Parameter flow from event payload into actions

- **Milestone: Trigger / Collision events** 🧭
  - Spatial trigger events (`trigger.enter`, `trigger.exit`)

- **Milestone: Deterministic core hooks** 🧭
  - Fixed tick loop for gameplay
  - Replayable input stream

## Non-goals (for now)
- Fully deterministic rendering/audio 🧭
- A complete visual scripting system 🧭

## Appendix: Implemented subset (today)

### Messages ✅
- `UiMessage::ButtonPressed(String)` — emitted when a UI button is pressed; trigger is the button's `action` field with `"ui."` prefix stripped
- `SceneEvent::{Requested(String), Loaded(String), Ready(String)}` — scene lifecycle
- `InputAction::{Move(Vec2), Turn(f32), Look(Vec2), Jump(bool), Run(bool)}` — abstract input
- `InputActionMessage { entity: Entity, action: InputAction }` — input bound to an entity

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

### Infrastructure ✅
- `ActionQueue` — push/pop queue processed each frame by `action_executor_system`
- `LogicState` resource — tracks the current named state (default `""`); checked by the interpreter when evaluating `when` guards
- `DebugState` resource — tracks `frame`, `app_state`, `last_action`, `scene`, `logic_state`; serialised to DOM on WASM for browser testing
- Data-defined rules loaded from `logic/rules.ron` via `LogicRulesAsset`; rules support optional `when: Option<String>` state guard

> New Messages or Actions must update `docs/STATUS.md` (Engine ABI section), this appendix, and `docs/20_data_formats.md` with an authoring example.


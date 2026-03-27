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

- ✅ A robust action layer exists: `ActionQueue` plus actions such as `LoadScene(String)`, `Quit`, `Log`, `Spawn`, and `PlayAnimation`.
- ✅ UI messages exist (`UiMessage`) and are emitted by UI button interaction; button `action` strings have the `"ui."` prefix stripped before firing (e.g. `action: "ui.dance"` → `UiMessage::ButtonPressed("dance")`).
- ✅ Input messages (`InputActionMessage`) decouple raw input from gameplay logic.
- ✅ Scene lifecycle events (`SceneEvent`) are emitted during loading transitions.
- ✅ A message interpreter maps UI messages to actions using data-defined rules loaded from `logic/rules.ron`.
- ✅ An action executor applies actions; notably:
  - `LoadScene(path)` loads a scene asset and transitions to `LoadingScene`.
  - `Quit` requests app exit (writes `AppExit::Success`).
  - `Spawn(path)` spawns a scene/model.
  - `PlayAnimation(clip)` plays an animation on available controllers.
  - `Log(msg)` emits an `info!` log line.
- ✅ `DebugState` resource exposes `last_action`, `app_state`, `scene`, and `frame` for observability and browser-based testing.

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

## Action model (planned)
Actions represent explicit operations the runtime can execute.

### Action categories

#### Scene actions 🧭
- `LoadScene(path)`
- `UnloadScene(name)`

#### App/system actions 🧭
- `Quit`

#### Entity actions 🧭
- `Spawn(template_id)`
- `Despawn(entity)`
- `SetTransform(entity, transform)`

#### Animation/audio actions 🧭
- `PlayAnimation(entity, clip, options)`
- `PlaySound(sound_id, options)`

#### State/variables actions 🧭
- `SetVar(key, value)`
- `IncVar(key, delta)`

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
        (
            on: "ui.button_pressed:start_game",
            do_actions: [ Log("Starting"), LoadScene("scenes/main.scene.ron") ],
        ),
        (
            on: "ui.button_pressed:quit",
            do_actions: [ Quit ],
        ),
    ],
)
```

Event name format: `"<domain>.<type>:<payload>"`. The interpreter matches the full string against each rule's `on` field. UI button events are always `"ui.button_pressed:<trigger>"` where the trigger is the button's `action` field with the `"ui."` prefix stripped.

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
  - ✅ Full action set: `LoadScene`, `Quit`, `Log`, `Spawn`, `PlayAnimation`
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
- `Quit` — writes `AppExit::Success`
- `Log(String)` — emits an `info!` log line
- `Spawn(String)` — spawns a model by asset path
- `PlayAnimation(String)` — plays an animation by semantic ID (see AnimationPolicy)

### Infrastructure ✅
- `ActionQueue` — push/pop queue processed each frame by `action_executor_system`
- `DebugState` resource — tracks `frame`, `app_state`, `last_action`, `scene`; serialised to DOM on WASM for browser testing
- Data-defined rules loaded from `logic/rules.ron` via `LogicRulesAsset`

> New Messages or Actions must update `docs/STATUS.md` (Engine ABI section), this appendix, and `docs/20_data_formats.md` with an authoring example.


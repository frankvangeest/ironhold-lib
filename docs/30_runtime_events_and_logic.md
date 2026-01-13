# Runtime: Events and Logic

> **Doc type:** Design Doc (vision)
>
> **Status legend:**
> - ✅ **Implemented** — exists in code today
> - 🧪 **Prototype / Partial** — exists but incomplete or unstable
> - 🧭 **Planned** — intended design; not implemented yet

## Status
🧭 Planned (with a small ✅ implemented subset)

## Goal
Provide a single, cross-platform logic model that is **driven by data** and does not require recompilation for most gameplay iteration. The runtime should be consistent between native and web/WASM.

## Design overview
Ironhold’s runtime model is built around three layers:

1. **Messages (events)** 🧭  
   Standardized, engine-level messages representing “something happened” (input, UI, triggers, scene lifecycle, animation markers).
2. **Actions** 🧭  
   Discrete, explicit commands representing “do something” (load scene, play animation, spawn entity, set variable, etc.).
3. **Execution** 🧭  
   A controlled executor that applies actions in a predictable order, enabling testing, determinism strategies, and clear debugging.

### Why separate messages and actions?
- Messages are **observations** (facts): a button was pressed, an input action fired, a trigger entered.
- Actions are **commands** (intent): load a scene, play a sound, set a state variable.

This separation helps:
- keep data-defined logic declarative and testable 🧭
- support deterministic simulation by controlling when/how actions apply 🧭
- decouple UI/input from gameplay logic 🧭

## Implementation snapshot (today)
This section is factual and reflects what exists right now.

- ✅ A minimal action layer exists: `Action::LoadScene(String)` plus `ActionQueue`.
- ✅ A UI message type exists (`UiMessage`) and is emitted by UI button interaction.
- ✅ The app wires message interpretation and action execution systems.

> Note: the broader event catalog and data-defined logic rules described below are not implemented yet.

## Event model (planned)
We standardize runtime messages so content can bind to them consistently.

### Core message categories

#### 1) InputAction 🧭
Abstract input actions (not raw keys/buttons):
- `input.move` (vector2)
- `input.look` (vector2)
- `input.jump` (pressed/released)

**Why:** input mappings vary by platform/device, but gameplay should consume stable action names.

#### 2) UiEvent 🧭
UI interactions and higher-level UI events:
- `ui.button_pressed` (id)
- `ui.menu_opened` (name)

**Why:** keep UI wiring declarative; bind UI events to gameplay actions.

#### 3) SceneEvent 🧭
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

### Example (planned, pseudo-RON)
```ron
(
  rules: [
    (
      on: "ui.start_game",
      do: [ (LoadScene: "scenes/main.ron") ],
    ),
  ],
)
```

> The exact schema is not finalized; this is illustrative.

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
This section ties design pieces to incremental implementation.

- **MVP (partially implemented today)** 🧪
  - ✅ UiEvent subset: button press → message
  - ✅ Action subset: `LoadScene`
  - 🧪 Interpreter/executor exist but support limited actions

- **Milestone: Event schema v1** 🧭
  - InputAction abstraction
  - Scene lifecycle events
  - Basic trigger events

- **Milestone: Rule bindings v1** 🧭
  - Data-defined event→action rules
  - Validation + diagnostics

- **Milestone: Deterministic core hooks** 🧭
  - Fixed tick loop for gameplay
  - Replayable input stream

## Non-goals (for now)
- Fully deterministic rendering/audio 🧭
- A complete visual scripting system 🧭

## Appendix: Implemented subset (today)
This list is intentionally short and should be updated when code expands.

- ✅ `Action::LoadScene(String)`
- ✅ `ActionQueue` (push/pop)
- ✅ UI button interaction emits `UiMessage::ButtonPressed(path)` which can request a scene load.



# Runtime: Events and Logic

## Goal
Provide a single, cross-platform logic model that is driven by data and does not require recompilation.

## Event model
We standardize engine-level messages:
- InputAction (abstracted inputs, not raw keys)
- UiEvent
- SceneEvent (requested/loaded/ready)
- Trigger/Collision
- AnimationMarker (optional)

Rules:
- Content references events by string (e.g. "ui.start.clicked"),
  engine compiles them to stable IDs at load for fast runtime matching.
- Payloads are restricted to deterministic-friendly primitives
  (bool/int/f32/vec2/vec3/string/entity ref).

## Logic model
We support:
- Global state machines (project-level)
- Per-entity state machines (behavior components on entities)

FSM transition structure:
- from_state
- on_event
- optional guards (conditions)
- actions on transition
- on_enter/on_exit actions

## Action model
Actions are the stable ABI between data logic and engine code:
- LoadScene(path)
- OpenUi(menu)
- PlayAnimation(entity, clip)
- SetVelocity(entity, vec3)
- SetVar(key, value)
- EmitEvent(event_id, payload)

## Scheduling
- Deterministic “truth” logic runs on a fixed tick schedule.
- Presentation runs per-frame and reads authoritative state.

Short-term: start with scene changes + UI actions using the Action bus.
``

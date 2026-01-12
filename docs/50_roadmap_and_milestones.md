
# Roadmap and Milestones

This file defines stable milestones and "feature-freeze" gates that ensure the core is stable before adding new features.

## Principles
- Each milestone produces a usable release.
- Each milestone includes: docs, tests, examples, and schema compatibility notes.
- Only add new gameplay features after the runtime model (events/actions/logic) is stable.

## Milestones (Beta series)

### Beta 0.1 — “Baseline Runtime”
Goal: current behavior stabilized and documented.
- Project+scene RON load works on native + web
- UI button can load scene
- Player controller + orbit camera + animations work via scene config
- Basic schema versioning and validation errors
Deliverables:
- docs added
- example project ships
- CI builds native + web

### Beta 0.2 — “Event/Action Bus”
Goal: decouple systems via Messages + Actions (no direct coupling).
- UI system emits UiMessage
- Scene manager listens and emits Scene events
- Actions exist as stable “engine ABI”
Deliverables:
- event and action list documented
- integration tests for scene switching
- no behavior change from 0.1 (refactor-only)

### Beta 0.3 — “Global Logic (FSM v1)”
Goal: project-level logic in data.
- Add global FSM asset and interpreter
- Start menu logic uses FSM (not hardcoded)
- Conditions + variables (minimal)
Deliverables:
- global logic examples
- validation tooling (clear errors for missing events/actions)

### Beta 0.4 — “Entity Logic (FSM v1)”
Goal: per-entity behaviors in data.
- Behavior component referencing FSM asset
- Trigger zones + interaction messages (enter/exit)
- At least one example: door/pickup/NPC idle-wander (simple)
Deliverables:
- examples + docs
- deterministic-friendly payload restrictions

### Beta 0.5 — “Deterministic Tick + Replay”
Goal: prepare for multiplayer.
- Fixed tick schedule for gameplay
- Deterministic RNG resource
- InputAction stream capture + replay (offline)
- Snapshot/restore for core gameplay state (minimal)
Deliverables:
- deterministic mode doc + constraints
- replay demo in native + web

### Beta 0.6 — “Networking Prototype”
Goal: prove architecture supports multiplayer.
- Start with server-authoritative minimal sync
- Inputs sent to server, server simulates, clients render with interpolation
- Basic connect/disconnect flow
Deliverables:
- network message protocol doc
- latency/jitter test harness
- multiplayer demo scene

## Release gates (must pass before bumping beta)
- schema versioning rules enforced
- docs updated
- examples updated
- CI green on native + web
- no known data-breaking changes without migration notes
``

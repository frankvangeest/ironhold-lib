
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
- UI system emits UiEvent
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

### Beta 0.7 — “Loading & Preloading”
Goal: eliminate frozen-screen loading and enable pre-warming of scenes.
- Engine-level loading overlay during `LoadingScene` / `LoadingProject` states
- `scene.loading_progress:{0-100}` milestone events from loader and terrain task
- `loading_scene` field in `ProjectConfig` for custom splash scenes
- `preload_poll_system`: watch `PreloadedScenes` handles, emit `scene.preloaded:{name}`
- `LoadScene` fast-path when handle is already in `PreloadedScenes`
Deliverables:
- loading overlay visible from first frame of loading until `InGame`
- custom splash scene support
- preload action documented and tested
Design: `planning/features/loading_screen.md`, `planning/features/scene_preloading.md`

## Release gates (must pass before bumping beta)
- schema versioning rules enforced
- docs updated
- examples updated
- CI green on native + web
- breaking RON changes follow the policy below

## Breaking change policy

The strictness scales with the release stage.

### Pre-1.0 (current)
Breaking RON schema changes are permitted without migration notes. Two conditions must be met in the same commit:
1. All `assets/projects/` example files are updated to the new shape — no broken examples on `main`.
2. The schema version is bumped on the affected file type when the shape changes.

No upgrade guide or deprecation cycle is required. The commit message is sufficient documentation. (The repo is public but there are no committed external users before 1.0.)

### Beta milestone boundaries (0.5, 0.6, …)
Add a short "Breaking changes in this milestone" paragraph to the milestone's backlog section or to the relevant feature file before merging. A bullet list of what changed and what to update in project files is enough — no full migration guide needed.

### 1.0 and beyond
Full migration notes required: a dedicated section in `docs/20_data_formats.md`, a schema version bump, and a one-paragraph upgrade summary in the milestone doc. Breaking changes must be flagged in the PR description. Deprecation cycle (warn one milestone, remove the next) preferred over silent removal.
``

See docs/STATUS.md for the authoritative, up‑to‑date implementation status.
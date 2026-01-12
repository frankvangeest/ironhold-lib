# plan.md — Ironhold-lib Roadmap, Architecture Decisions, and TODO

> This document is the **implementation plan** for Ironhold-lib. It is intentionally practical: what we build, in what order, and why.
> 
> For deeper design docs (as they get added), see `docs/`.

---

## 0) Goals (what we are building)

Ironhold-lib is a **cross-platform** (Native + Web/WASM) Bevy 0.17 runtime where games are defined by:
- `assets/project.ron` (project-level config)
- `assets/scenes/*.ron` (scene-level config)
- assets (models, textures, audio)

**Game creators should not recompile the engine** to create new projects/scenes.

The engine provides **capability building blocks** (controller, camera, animation, UI, etc.).
How they are used is **data-defined**.

Future requirement: **networked multiplayer**.

---

## 1) Current State (baseline)

What exists today (baseline functionality):
- Project config loads from `assets/project.ron` and points to `initial_scene`.
- Scene config loads from `assets/scenes/*.ron` and spawns:
  - models from `.glb`
  - UI (Button -> LoadScene)
  - optional player with configurable input mapping
  - orbit camera
  - animation mapping and playback from GLTF clips
- Cross-platform runners:
  - `ironhold_native` calls `ironhold_core::start_app()`
  - `ironhold_web` exposes a WASM entrypoint via wasm-bindgen

This is **Beta 0.1 material** once documented and stabilized.

---

## 2) Direction: Target Architecture

We are evolving from “systems directly doing things” to a stable runtime model:

**Messages (events) → Interpreter (data logic) → Actions → Executors**

### Why this structure?
- Keeps the engine generic and reusable.
- Makes behavior data-driven (RON), so creators don’t recompile.
- Avoids tight coupling (e.g., UI shouldn’t directly manage scene loading).
- Prepares the engine for deterministic simulation and multiplayer.

### Layering

**A) App-level flow (global lifecycle)**
- Use Bevy `States` for: Boot / Loading / InGame / Paused / Error.

**B) Data-driven logic layer**
- Global logic: project-level state machine(s) (menus, flow, quests).
- Entity logic: per-entity machines (interactables, NPCs).

**C) Capability modules**
- Systems that emit messages (input, UI, triggers, collisions).
- Systems that execute actions (move, play animation, load scene, open UI).

---

## 3) Determinism Strategy (for multiplayer readiness)

We do **not** require the entire engine to be deterministic from day 1.

Instead we split:

- **Deterministic gameplay core** (“truth”)
  - fixed tick
  - stable update order
  - deterministic RNG
  - input stream model
  - snapshot/restore hooks (for replay/rollback)

- **Non-deterministic presentation**
  - camera smoothing
  - particles/VFX
  - animation blending (as long as it’s driven from deterministic state)

This keeps rollback and deterministic networking options open without freezing feature development too early.

---

## 4) Implementation Order (recommended)

### Phase A — Documentation + Refactor (no behavior changes)
**Goal:** lock down the baseline and create the runtime backbone.

1. Add `docs/` structure (overview, architecture, formats, roadmap).
2. Update `README.md` to point to docs.
3. Refactor `ironhold_core/src/lib.rs` into modules:
   - `schema/` (project/scene types)
   - `runtime/` (messages/actions/interpreter)
   - `capabilities/` (player/camera/animation/ui)
4. Introduce an **Action model**:
   - `Action` enum + `ActionQueue` resource
   - start with `Action::LoadScene(String)`
5. Convert UI button handling:
   - UI system emits `UiMessage`
   - Scene manager interprets to `Action::LoadScene`
   - Scene manager executes the action

**Why first?** This prevents “feature spaghetti” and makes new features easier to add safely.

---

### Phase B — Event/Action Bus Stabilization
**Goal:** decouple subsystems; enforce stable internal contracts.

6. Standardize message types:
   - `UiMessage`
   - `SceneRequestMessage`, `SceneLoadedMessage`, `SceneReadyMessage`
   - `InputActionMessage`
7. Add validation:
   - missing assets paths
   - invalid key names
   - unknown actions

---

### Phase C — Global Logic (FSM v1)
**Goal:** project-level logic in data.

8. Add `GlobalLogic` asset format (RON):
   - FSM with transitions on events
   - guards + actions
9. Add a global interpreter system:
   - consumes messages
   - updates global machine state
   - emits actions
10. Move menu flow into global FSM:
   - start-menu button triggers UiMessage
   - FSM decides to LoadScene

---

### Phase D — Entity Logic (FSM v1)
**Goal:** per-entity behaviors in data.

11. Add `BehaviorMachine` component that references an FSM asset.
12. Add Trigger capability:
   - trigger volumes emit `TriggerEntered/Exited` messages
13. Example behaviors:
   - door open/close
   - pickup

---

### Phase E — Deterministic Tick + Replay
**Goal:** multiplayer foundations.

14. Move authoritative gameplay updates to fixed tick schedule.
15. Add deterministic RNG resource.
16. Add input capture + replay:
   - record InputAction stream
   - replay on demand
17. Add minimal snapshot/restore for core state (replay first).

---

### Phase F — Networking Prototype
**Goal:** prove architecture supports multiplayer.

18. Server-authoritative prototype (recommended first):
   - client sends inputs
   - server simulates authoritative core
   - server sends state snapshots/deltas
19. Add network testing harness (latency/jitter).

---

## 5) Beta Milestones (stability gates)

We want stable beta releases before adding lots of new capabilities.

### Beta 0.1 — Baseline Runtime
- Native + Web parity
- Project + scene loading
- UI button loads a scene
- Player controller + orbit camera + animation mapping
- Docs added

### Beta 0.2 — Event/Action Bus
- UI emits UiMessage (no direct scene load)
- Scene manager consumes messages and executes actions
- Actions documented as “engine ABI”
- No functional behavior changes from 0.1 (refactor-only)

### Beta 0.3 — Global Logic (FSM v1)
- Global logic FSM asset + interpreter
- Start menu logic driven by FSM
- Minimal conditions + variables

### Beta 0.4 — Entity Logic (FSM v1)
- Per-entity behavior component
- Trigger messages
- Example interactable (door/pickup)

### Beta 0.5 — Deterministic Tick + Replay
- Fixed tick authoritative core
- Deterministic RNG
- Input capture + replay
- Snapshot/restore hooks

### Beta 0.6 — Networking Prototype
- Server-authoritative minimal multiplayer
- Basic connect/disconnect
- Snapshot/delta sync

---

## 6) Release Gates (must-pass checklist)

Before bumping a beta version:
- Docs updated (architecture, formats, roadmap)
- Example project updated
- CI builds for native + web
- Schema changes include versioning and migration notes
- Clear error messages for invalid RON

---

## 7) Short-Term TODO (next 2–3 weeks)

### Documentation
- [x] Add `docs/00_overview.md`
- [x] Add `docs/10_architecture.md`
- [x] Add `docs/20_data_formats.md`
- [x] Add `docs/30_runtime_events_and_logic.md`
- [x] Add `docs/40_determinism_and_networking.md`
- [x] Add `docs/50_roadmap_and_milestones.md`
- [x] Add `docs/60_contributing.md`
- [x] Update `README.md` links

### Refactor
- [x] Split `ironhold_core/src/lib.rs` into modules
- [x] Add `Action` enum + `ActionQueue`
- [x] Change UI button system to emit `UiMessage`
- [x] Add `SceneManager` to convert messages → `Action::LoadScene`

### Tests
- [ ] Add integration test: start-menu button triggers LoadScene
- [ ] Add RON validation tests (missing fields, invalid key names)

---

## 8) Notes / Open Decisions

- **Networking model choice:** start with server-authoritative; keep rollback possible.
- **Physics strategy:** keep character controller deterministic; evaluate full physics determinism later.
- **Schema versioning:** add `schema_version` to `ProjectConfig` and `GameLevel` as soon as possible.


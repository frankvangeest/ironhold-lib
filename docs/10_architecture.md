
# Architecture

## Current state (today)
- `ironhold_core`: core Bevy plugin(s), RON asset types, scene spawning, player controller, orbit camera, animation mapping, UI button -> scene load.
- `ironhold_native`: desktop runner calling `ironhold_core::start_app()`.
- `ironhold_web`: WASM runner exposing `start()` via wasm-bindgen.

Assets:
- `assets/project.ron`: selects initial scene.
- `assets/scenes/*.ron`: describe the scene contents (models, player config, UI).

## Target architecture (direction)
We evolve from "systems directly do things" to a stable, scalable runtime:

**Messages (events) → Interpreter (data logic) → Actions → Executors**

- **Event producers** (input/UI/triggers/etc.) emit Messages.
- **Interpreter** reads Messages + current state (global/per-entity) and emits Actions.
- **Executors** apply Actions via capability systems.

Why:
- Enables data-defined behavior without recompiling.
- Decouples features (UI doesn’t hardcode scene management).
- Prepares the engine for deterministic simulation and multiplayer later.

## Layering
### App-level flow (global)
Use Bevy app States for lifecycle:
Boot → LoadingProject → LoadingScene → InGame → Paused / Error

### Gameplay logic (data-driven)
- Global logic: “project-level” state machine(s) (e.g., menus, cutscenes).
- Entity logic: behavior machines attached to entities (e.g., door logic, NPC logic, locomotion).

### Capabilities
Capability modules provide:
- event sources (e.g., input mapping)
- action executors (e.g., Move, PlayAnimation, LoadScene)
- data schemas and validation

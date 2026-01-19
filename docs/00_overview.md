# Ironhold-lib: Overview

> **Doc type:** Design + Overview
>
> **Status legend:**
> - ✅ **Implemented** — exists in code today
> - 🧪 **Prototype / Partial** — exists but incomplete or unstable
> - 🧭 **Planned** — intended design; not implemented yet
>
## What is Ironhold-lib?

Ironhold-lib is a cross-platform (native + web/WASM) game runtime built on **Bevy**. The core goal is to enable **data-defined games**: creators can build and iterate on projects and scenes using data files (RON) and assets, without recompiling the engine for most content changes. 

## Goals (vision)

1. **Data-driven gameplay** 🧭  
   Most game behavior should be declared in data files (RON), not hard-coded.
2. **Single runtime model** 🧭  
   A shared, cross-platform logic layer that behaves the same on native and web.
3. **Composable capabilities** 🧭  
   Reusable “capability blocks” (player controller, cameras, UI flows, triggers, etc.) that can be enabled/configured by data.
4. **Schema evolution** 🧭  
   Versioned data formats with validation and migration paths.
5. **Determinism-ready foundation** 🧭  
   A path toward deterministic simulation for multiplayer/rollback, without requiring determinism everywhere from day one.

## Big ideas

### 1) Data-defined games (RON)

**Target design (planned):**
- A project file defines global configuration (initial scene, input mappings, UI roots, etc.). 🧭
- Scene files define entities to spawn, their capabilities, and their bindings to events/actions. 🧭
- Assets (models/textures/audio) are referenced by paths/handles from scene data. 🧭

**Implementation snapshot (today):**
- ✅ RON asset loading is wired up for `ProjectConfig` and `GameLevel` via `RonAssetPlugin`. 
- ✅ The project config currently includes at least `initial_scene` (see `ProjectConfig`). 

### 2) Capability blocks

**Target design (planned):**
- A capability is a reusable feature module with:
  - declared inputs/events it consumes 🧭
  - actions it emits/executes 🧭
  - validation rules for its configuration 🧭
- Capabilities are activated/configured by scene data.

**Implementation snapshot (today):**
- 🧪 Capability systems exist and are registered (player movement, orbit camera, animation playback), but the formal “capability registry + declarative bindings” is not fully implemented yet. 

### 3) Messages → Actions → Execution

**Target design (planned):**
- Standardized runtime messages/events (UI, input, scene lifecycle, triggers, animation markers, etc.). 🧭
- An interpreter maps messages to actions using data-defined rules. 🧭
- An action executor applies actions in a controlled, testable way. 🧭

**Implementation snapshot (today):**
- ✅ A message type (`UiMessage`) is registered and used by the UI button system. 
- ✅ A minimal action layer exists with `Action::LoadScene(String)` and an `ActionQueue`. 
- ✅ UI button presses emit a message which can drive a scene load request (via `UiAction::LoadScene` → `UiMessage::ButtonPressed`). 

## Repository layout

- `crates/ironhold_core` — core runtime plugin(s), schemas, runtime systems, and capability systems. ✅ 
- `crates/ironhold_native` — desktop runner calling `ironhold_core::start_app()`. ✅ 
- `crates/ironhold_web` — WASM runner exporting `start()` via `wasm-bindgen`. ✅ 
- `assets/` — example project + scenes + models. ✅ 
- `docs/` — design docs, data format spec drafts, roadmap, contributing guide. ✅ 

## Current implementation snapshot (today)

This section is intentionally brief and factual.

- ✅ The Bevy app is built by `start_app()` and adds `GamePlugin`. 
- ✅ Project configuration is loaded as an asset and transitions into a loading state. 
- ✅ UI buttons can trigger scene load requests. 
- ✅ Action queue infrastructure exists with a minimal `LoadScene` action. 
- 🧪 The richer event catalog described in the design docs is not implemented yet (beyond the current UI/scene loading flow). 

## Planned next steps (high level)

- 🧭 Expand and formalize the runtime **event schema** (input abstraction, scene lifecycle events, triggers/collisions, animation markers).
- 🧭 Move from ad-hoc wiring to **data-defined bindings** (strings → events/actions) with validation.
- ✅ Add **schema_version** to all data formats.
- 🧭 Add **schema_version migration notes**.
- 🧭 Introduce a **fixed-tick simulation loop** suitable for deterministic gameplay where needed.

## Where to read next

- `docs/10_architecture.md` — current state + target architecture
- `docs/20_data_formats.md` — spec draft for project/scene formats
- `docs/30_runtime_events_and_logic.md` — planned event/action model
- `docs/50_roadmap_and_milestones.md` — milestones and feature gates


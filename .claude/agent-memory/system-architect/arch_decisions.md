---
name: core-architectural-decisions
description: Foundational architecture decisions for ironhold-lib with rationale; consult before advising on any structural change
metadata:
  type: project
---

## Three-crate split

`ironhold_core` (platform-agnostic) / `ironhold_native` (desktop runner) / `ironhold_web` (WASM runner). Core must never contain `#[cfg(target_arch = "wasm32")]` guards or platform-specific imports — if you see one, it belongs in native or web. The runners are intentionally thin: they only parse CLI args (native) or call `wasm_bindgen(start)` (web) and delegate everything to `core::start_app()`.

**Why:** A single binary target for both desktop and WASM with zero shared source duplication. Core must compile cleanly for both targets with no conditional compilation.

## Message → Interpreter → Action → Executor pipeline

All game behavior flows: capability emits message → interpreter matches rules.ron/state_machine.ron → pushes Action to ActionQueue → executor dispatches Action. No capability system may push directly to ActionQueue — it must emit a typed message instead. The interpreter is the only code that writes to ActionQueue.

**Why:** Makes all behavior configurable from RON without recompiling. If a capability hardwires an action, the designer loses the ability to change that behavior.

## ActionQueue is FIFO

`ActionQueue` uses `VecDeque::pop_front()` — push order equals execution order. The FSM naturally pushes exit actions before entry actions; do not rely on any other ordering guarantee.

**Why:** Predictable execution order for state transitions, especially when exit and entry actions interact with the same entities.

## Asset paths via AssetCatalog only

All asset paths live in `assets/projects/{name}/assets.ron`, resolved through `LoadedAssetCatalog`. Capabilities receive catalog keys (strings), never file paths. The only code that resolves catalog keys to paths is `LoadedAssetCatalog::get_*()`.

**Why:** Ensures all assets are auditable (CLI can validate them), avoids stale paths, and makes projects self-contained.

## Schema is the designer's API surface

`crates/ironhold_core/src/schema/` contains all RON-serializable types. Every type a designer might author in a RON file must be reachable from a schema type with `#[derive(Deserialize)]`. If a type is not in schema/, it is invisible to designers.

**Why:** The schema layer is the stable contract. Runtime internals can be refactored freely; schema changes are breaking API changes.

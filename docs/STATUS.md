# STATUS — Feature Status Matrix

> **Purpose:** single source of truth for what is **implemented today** vs **planned**.
>
> **Status legend:**
> - ✅ **Implemented** — exists in code today
> - 🧪 **Prototype / Partial** — exists but incomplete or unstable
> - 🧭 **Planned** — intended design; not implemented yet

---

## Runtime (Messages → Actions → Execution)

- ✅ **Scene load flow via UI**: UI button interaction emits a UI message and can drive a scene load request.
- ✅ **Action infrastructure**: `ActionQueue` exists with a minimal `LoadScene` action.
- 🧪 **Interpreter/executor pipeline**: systems are wired, but only a small subset of actions/messages exist.
- 🧭 **Unified event schema**: InputAction, SceneEvent, Trigger/Collision, AnimationMarker, etc.
- 🧭 **Data-defined rules**: declarative Event→Action bindings authored in RON.

## Scenes & Content

- ✅ **RON asset loading**: project config and levels/scenes load as assets.
- 🧪 **Scene lifecycle**: explicit requested/loaded/ready events are planned.
- 🧭 **Scene composition schema v1**: templates, prefabs, tags, triggers.

## Capabilities

- ✅ **Capability systems exist**: player movement, orbit camera, animation playback are present and registered.
- 🧪 **Configuration via RON**: partially supported; formal binding + validation planned.
- 🧭 **Capability registry**: declare events/actions/validation rules per capability.

## Data formats & Validation

- ✅ **Current schema structs**: minimal `ProjectConfig` (initial scene) and level/scene schema types exist.
- 🧪 **RON validation tests**: basic validation exists via tests.
- ✅ **schema_version** everywhere
- 🧭 **schema migrations**
- 🧭 **Strict validation + diagnostics** for content authors.

## Determinism & Networking

- 🧭 **Fixed-tick deterministic gameplay loop**.
- 🧭 **Replay tooling** (record inputs per tick, replay, state hashing).
- 🧭 **Networking prototypes** (lockstep → prediction/reconciliation → rollback).

## Platforms

- ✅ **Native runner**: desktop app runner exists.
- ✅ **Web runner**: WASM entry point exists.
- 🧪 **Platform parity checks**: automated parity/replay tests planned.

---

## Update policy

- Update this file whenever:
  - you add/remove an `Action` or message type
  - you change any schema structs that affect user-authored RON
  - you complete a milestone item in the roadmap


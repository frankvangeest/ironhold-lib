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
- ✅ **Quit flow via UI**: UI button interaction can emit a quit message and request application exit.
- ✅ **Action infrastructure**: `ActionQueue` exists with actions including `LoadScene` and `Quit`.
- ✅ **Interpreter/executor pipeline**: UI messages are interpreted into actions and executed.
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

- ✅ **Top-level schema versioning**: `schema_version` is required for top-level project + scene files.
- ✅ **Strict deserialization**: unknown fields are rejected for top-level formats.
- ✅ **RON validation tests**: enforce required `schema_version`, reject unknown fields, and validate supported schema versions for `ProjectConfig` and `GameLevel`.
- ✅ **Asset regression test**: scans `assets/*.ron` and `assets/scenes/**/*.ron` to ensure every file parses and validates (prevents future regressions).
- 🧭 **Schema migrations**
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



==== SUMMARY: 45 files, 238158 bytes raw ====

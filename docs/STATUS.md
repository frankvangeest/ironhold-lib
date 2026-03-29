# Project Status

_Last updated: 2026‑03‑27_

## Legend
- ✅ Implemented
- 🟡 In progress / Partial
- ⛔ Planned (not implemented)

---

## Milestones

| Milestone | Name                              | Status | Notes |
|----------:|-----------------------------------|:------:|-------|
| 0.1       | Baseline Runtime                  |   ✅   | Native+web parity; RON project/scene load; UI button → scene load; player/camera/animation; schema v1; validation tests. |
| 0.2       | Event/Action Bus refactor         |   ✅   | Message→Interpreter→Action→Executor fully wired with project-level logic rules. |
| 0.3       | Global Logic (FSM v1)             |   ⛔   | Not implemented. |
| 0.4       | Entity Logic (FSM v1)             |   ⛔   | Not implemented. |
| 0.5       | Deterministic Tick + Replay       |   ⛔   | Not implemented. |
| 0.6       | Networking Prototype              |   ⛔   | Not implemented. |

---

## Feature Matrix (Today)

### Runtime & Logic
| Area                          | Status | Notes |
|-------------------------------|:------:|-------|
| UI → logic trigger            |   ✅   | Button press → `UiMessage` → Project Rule matched → Action(s) queued. |
| Logic → Action execution      |   ✅   | `ActionQueue` processed by `action_executor_system`. |
| Action infrastructure         |   ✅   | interpreter & executor wired. |
| Live event domains            |   ✅   | `UiMessage`, `SceneEvent`, `InputAction`/`InputActionMessage` are live. |
| Planned event domains         |   ⛔   | (AI, interaction, dialogue, networking) are planned. |
| Scene lifecycle events        |   🟡   | `Requested/Loaded/Ready` types exist; full lifecycle choreography is WIP. |

### Data Formats & Validation
| Area                                  | Status | Notes |
|---------------------------------------|:------:|-------|
| Top‑level `schema_version` required   |   ✅   | Enforced for Project & Scene (v1). |
| Deny unknown fields                   |   ✅   | Strict serde on top‑level assets. |
| Asset regression tests                |   ✅   | Scans `assets/**/*.ron` for schema compliance. |
| Schema migrations/diagnostics         |   ⛔   | Planned. |

### Capabilities
| Area                        | Status | Notes |
|-----------------------------|:------:|-------|
| Player movement             |   ✅   | Data‑configured via scene `player` block. |
| Orbit camera                |   ✅   | Data‑configured via `player.camera`. |
| Animation playback          |   ✅   | Data‑configured via `player.animations`. |
| Terrain rendering           |   ✅   | WebGPU compatible heightmap and splatmap based terrain. |
| HDR Lighting (IBL)          |   ✅   | Scene `lighting` (Ambient, Directional, Environment) & Project fallback. |
| Capability registry         |   ⛔   | Planned (declare events/actions/validation per capability). |

### Platforms
| Area                  | Status | Notes |
|-----------------------|:------:|-------|
| Native runner         |   ✅   | `crates/ironhold_native` (CLI can select project with `--project <name>`). |
| Web runner (WASM)     |   ✅   | `crates/ironhold_web`; project selectable via `?project=<name>` URL param. |
| Platform parity tests |   ✅   | Headless Chromium browser test suite (`test_web.py`) covering all three example projects. |

---

## Engine ABI (today)

### Messages
- `UiMessage::ButtonPressed(String)`
- `SceneEvent::{Requested(String), Loaded(String), Ready(String)}`
- `InputAction::{Move(Vec2), Turn(f32), Look(Vec2), Jump(bool), Run(bool)}`
- `InputActionMessage { entity, action: InputAction }`

### Actions
- `Action::LoadScene(String)`
- `Action::Quit`
- `Action::Log(String)` — logs a message (debug/telemetry)
- `Action::Spawn(String)` — spawns an entity by asset id/path
- `Action::PlayAnimation(String)` — plays a named animation/clip
- `Action::PlaySound(String)` — plays a sound by audio catalog key; warns and no-ops for unsupported formats or missing keys

> New Messages/Actions **must** update this table and include examples + tests.

### Debug / Test Surface
- `DebugState` resource — updated every `PostUpdate` frame; exposes `frame`, `app_state`, `last_action`, `scene`.
- On WASM, `DebugState` is serialised as JSON into `<div id="debug-state">` by `sync_debug_state_to_dom`, making it readable by browser automation tools.

---

## Project Logic (rules)
- Project files map events to actions.
- Example:
  ```ron
  rules: [
    (
      on: "ui.button_pressed:start_game",
      do_actions: [ Log("Starting"), LoadScene("scenes/main.ron") ],
    ),
  ]
  ```

## UI v1 Scope (authoring)
- Supported element: `Button { text, action: Trigger(String), position: Option<(f32, f32)> }`
- Example:
  ```ron
  ui: [
    Button(
      text: "Start Game", 
      action: Trigger("start_game"), 
      position: Some((100.0, 100.0)) // Optional, defaults to None (centered)
    ),
    Button(
      text: "Quit",       
      action: Trigger("quit")
    ),
  ]
  ```

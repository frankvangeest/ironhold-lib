# Project Status

_Last updated: 2026‑04‑01_

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
| 0.3       | Global Logic (FSM v1)             |   🟡   | Named logic states (`LogicState`), state-gated rules (`when`), and `EnterState` action implemented. Proper FSM asset type with explicit states, transitions, and entry/exit actions not yet implemented. |
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
| State-gated rules             |   ✅   | `LogicRule.when` field gates rules to a named logic state; `EnterState` action transitions between states. |
| Live event domains            |   ✅   | `UiMessage`, `SceneEvent`, `InputAction`/`InputActionMessage` are live. |
| Planned event domains         |   ⛔   | (AI, interaction, dialogue, networking) are planned. |
| Scene lifecycle events        |   🟡   | `Requested/Loaded/Ready/Unloading` types exist; full lifecycle choreography is WIP. |

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
- `Action::LoadScene(String)` — loads a `.scene.ron`, replaces current scene
- `Action::LoadSceneOverlay(String)` — loads a `.scene.ron` as an overlay (e.g. pause menu)
- `Action::UnloadOverlay` — despawns all overlay entities
- `Action::ToggleOverlay(String)` — opens overlay if none active, closes if one is active
- `Action::Quit` — requests app exit
- `Action::Log(String)` — logs a message (debug/telemetry)
- `Action::Spawn { prefab, id }` — spawns a prefab instance; auto-generates ID if `id` is omitted
- `Action::Despawn(String)` — removes a previously spawned entity by its spawn ID
- `Action::PlayAnimation(String)` — plays a named animation/clip
- `Action::PlaySound(String)` — fire-and-forget sound by audio catalog key; warns for unsupported formats or missing keys
- `Action::PlayMusicLoop(String)` — starts a looping background music track by audio catalog key
- `Action::StopMusic` — stops the current background music track
- `Action::SetVolume(u32)` — sets global volume 0–100
- `Action::Preload(String)` — warms the asset cache for a `.scene.ron` before it is needed
- `Action::EnterState(String)` — transitions the interpreter to a named logic state; empty string returns to stateless (always-fire) default

> New Messages/Actions **must** update this table and include examples + tests.

### Debug / Test Surface
- `DebugState` resource — updated every `PostUpdate` frame; exposes `frame`, `app_state`, `last_action`, `scene`, `logic_state`.
- On WASM, `DebugState` is serialised as JSON into `<div id="debug-state">` by `sync_debug_state_to_dom`, making it readable by browser automation tools.
  ```json
  {"frame": 42, "app_state": "InGame", "last_action": "EnterState(\"playing\")", "scene": "...", "logic_state": "playing"}
  ```

---

## Project Logic (rules)
- Project files map events to actions, with optional state-gating.
- Rules with `when` omitted (or `None`) fire in any logic state.
- Rules with `when: Some("state_name")` only fire while the interpreter is in that named state.
- `Action::EnterState(name)` transitions to a named state; `EnterState("")` returns to stateless.
- Example:
  ```ron
  rules: [
    // State transition — no when guard, fires always
    ( on: "scene.ready:main", do_actions: [ EnterState("playing"), PlayMusicLoop("bg_music") ] ),

    // Gated to "playing" state
    ( on: "ui.button_pressed:dance", when: Some("playing"), do_actions: [ PlayAnimation("dance") ] ),
    ( on: "ui.button_pressed:toggle_pause", when: Some("playing"), do_actions: [ LoadSceneOverlay("scenes/pause.scene.ron"), EnterState("paused") ] ),

    // Gated to "paused" state
    ( on: "ui.button_pressed:toggle_pause", when: Some("paused"), do_actions: [ UnloadOverlay, EnterState("playing") ] ),
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

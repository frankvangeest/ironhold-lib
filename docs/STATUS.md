# Project Status

_Last updated: 2026‑04‑17_

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
| 0.3       | Global Logic (FSM v1)             |   ✅   | Named logic states, state-gated rules, `EnterState`, and full `StateMachineAsset` (states, transitions, entry/exit, in-state `on`, `global_on`, any-state transitions). `3rd_person_game_demo` migrated to FSM. |
| 0.4       | Entity Logic (FSM v1)             |   ⛔   | Not implemented. |
| 0.5       | Deterministic Tick + Replay       |   ⛔   | Not implemented. |
| 0.6       | Networking Prototype              |   ⛔   | Not implemented. |

---

## Feature Matrix (Today)

### Runtime & Logic
| Area                          | Status | Notes |
|-------------------------------|:------:|-------|
| UI → logic trigger            |   ✅   | Button press → `UiEvent` → Project Rule matched → Action(s) queued. |
| Logic → Action execution      |   ✅   | `ActionQueue` processed by `action_executor_system`. |
| Action infrastructure         |   ✅   | interpreter & executor wired. |
| State-gated rules             |   ✅   | `LogicRule.when` field gates rules to a named logic state; `EnterState` action transitions between states. |
| FSM asset (`StateMachineAsset`) |   ✅   | `logic/state_machine.ron` — states with entry/exit/on, transitions (any-state or from-specific), `global_on`; replaces `rules.ron` for FSM projects. |
| Live event domains            |   ✅   | `UiEvent`, `GameEvent`, `SceneEvent`, `InputAction`/`InputActionMessage` are live. |
| Planned event domains         |   ⛔   | (AI, interaction, dialogue, networking) are planned. |
| Scene lifecycle events        |   🟡   | `Requested/Loaded/Ready/Unloading` types exist; full lifecycle choreography is WIP. |

### Data Formats & Validation
| Area                                  | Status | Notes |
|---------------------------------------|:------:|-------|
| Top‑level `schema_version` required   |   ✅   | Project: v1/v2/v3 accepted; Scene: v2. |
| Deny unknown fields                   |   ✅   | Strict serde on top‑level assets. |
| Asset regression tests                |   ✅   | Scans `assets/**/*.ron` for schema compliance. |
| Schema migrations/diagnostics         |   ⛔   | Planned. |

### Capabilities
| Area                        | Status | Notes |
|-----------------------------|:------:|-------|
| Player movement             |   ✅   | Data‑configured via scene `player` block. |
| Orbit camera                |   ✅   | Data‑configured via `player.camera`. |
| Animation playback          |   ✅   | Data‑configured via `player.animations`. |
| Fly camera                  |   ✅   | Free-flying camera; spawned via `"flycam"` tag on prefab. LMB/RMB hold to look, WASD move, Shift fast mode. Optional `flycam_position` UI label. |
| NPC AI                      |   ✅   | `NpcAgent` component; states: Idle → Patrol → Alerted → Chase/Flee/Interact → Return. FOV + optional Rapier line-of-sight check. Emits `"npc.player_spotted:{id}"`, `"npc.player_reached:{id}"`, `"npc.player_lost:{id}"` triggers. |
| Collectible triggers        |   ✅   | `Collectable` component on Rapier sensor; on player overlap emits `GameEvent::Trigger("entity.collected:{spawn_id}")`. Response (Despawn, AddScore, etc.) is configured in RON. |
| Motion (rotate/bob)         |   ✅   | `Motion` component; world-space continuous rotation (per-axis rad/s) and sinusoidal vertical bob (amplitude, frequency). Runs in `Update`; purely visual. |
| Custom WGSL material        |   ✅   | `CustomMaterial`; designer-supplied `.wgsl` fragment shader; 4×Vec4 uniform slots + up to 4 texture slots. See `docs/25_custom_shaders.md`. |
| Primitive shapes            |   ✅   | `kind: "primitive"` prefabs; Cuboid, Sphere, Cylinder, Capsule3d, Cone, Torus, ConicalFrustum. Dimensions and color configurable per-prefab and via `primitive_default_color` in project config. |
| Terrain rendering           |   ✅   | WebGPU compatible heightmap and splatmap based terrain. |
| IBL / Environment Lighting  |   ✅   | Scene `lighting` (Ambient, Directional, Environment) & Project fallback. HDR camera mode and bloom are excluded — see `docs/20_data_formats.md`. |
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
- `UiEvent::ButtonPressed(String)` — emitted by UI buttons and key bindings; flows into the rules pipeline as `"ui.button_pressed:{trigger}"`
- `GameEvent::Trigger(String)` — emitted by gameplay capabilities (physics sensors, etc.); flows into the rules pipeline as-is (the name is already namespaced, e.g. `"entity.collected:coin_01"`)
- `SceneEvent::{Requested(String), Loaded(String), Ready(String), Unloading(String)}` — scene lifecycle
- `InputAction::{Move(Vec2), Turn(f32), Look(Vec2), Jump(bool), Run(bool)}` — abstract input (point-to-point, not pipeline)
- `InputActionMessage { entity, action: InputAction }` — input bound to a specific entity (point-to-point, not pipeline)

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
- `Action::SetVolume(u8)` — sets global volume 0–100
- `Action::Preload(String)` — warms the asset cache for a `.scene.ron` before it is needed
- `Action::EnterState(String)` — transitions the interpreter to a named logic state; empty string returns to stateless (always-fire) default
- `Action::AddScore(i32)` — adds (or subtracts if negative) to `DebugState.score`; visible in DOM on WASM

> New Messages/Actions **must** update this table and include examples + tests.

### Debug / Test Surface
- `DebugState` resource — updated every `PostUpdate` frame; exposes `frame`, `app_state`, `last_action`, `scene`, `logic_state`, `score`.
- On WASM, `DebugState` is serialised as JSON into `<div id="debug-state">` by `sync_debug_state_to_dom`, making it readable by browser automation tools.
  ```json
  {"frame": 42, "app_state": "InGame", "last_action": "EnterState(\"playing\")", "scene": "...", "logic_state": "playing", "score": 0}
  ```

---

## Project Logic

Two authoring workflows are supported. A project uses one or the other via its `.project.ron`.

### rules.ron workflow (schema v2)
- `rules_path: Some("logic/rules.ron")` in project config.
- Rules with `when` omitted (or `None`) fire in any logic state.
- Rules with `when: Some("state_name")` only fire while the interpreter is in that named state.
- `Action::EnterState(name)` transitions to a named state; `EnterState("")` returns to stateless.

### state_machine.ron workflow (schema v1) ✅
- `state_machine_path: Some("logic/state_machine.ron")` in project config.
- Declares named states with `entry_actions`, `exit_actions`, and in-state `on` event bindings.
- `transitions` list drives state changes (`from` optional — omit for any-state transitions).
- `global_on` list fires regardless of current state without changing state.
- The engine fires exit/entry actions automatically; authors do not write `EnterState` manually.
- `initial_state` sets the starting `LogicState` immediately when the asset loads.
- Example:
  ```ron
  // logic/state_machine.ron
  (
    schema_version: 1,
    initial_state: "menu",
    global_on: [],
    states: [
      ( name: "menu", entry_actions: [], exit_actions: [],
        on: [ ( event: "ui.button_pressed:start_game", do_actions: [ LoadScene("scenes/main.scene.ron") ] ) ] ),
      ( name: "playing",
        entry_actions: [ PlayMusicLoop("bg_music") ],
        exit_actions:  [ StopMusic ],
        on: [ ( event: "ui.button_pressed:dance", do_actions: [ PlayAnimation("dance") ] ) ] ),
      ( name: "paused",
        entry_actions: [ LoadSceneOverlay("scenes/pause.scene.ron") ],
        exit_actions:  [ UnloadOverlay ],
        on: [] ),
    ],
    transitions: [
      ( on: "scene.ready:main",       to: "playing" ),  // any-state
      ( from: Some("playing"), on: "ui.button_pressed:toggle_pause", to: "paused" ),
      ( from: Some("paused"),  on: "ui.button_pressed:toggle_pause", to: "playing" ),
    ],
  )
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

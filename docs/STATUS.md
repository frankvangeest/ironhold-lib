# Project Status

_Last updated: 2026‑05‑18_

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
| 0.4       | Entity Logic (FSM v1)             |   ✅   | Per-entity `StateMachineAsset` behaviors (`.behavior.ron`), `{self}` substitution, `entity_fsm_interpreter_system`, `TriggerZone` and `Interactable` capabilities, `PlayAnimationOn`/`EmitEvent` actions. `entity_logic_demo` example project. |
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
| Live event domains            |   ✅   | `UiEvent`, `GameEvent`, `SceneEvent`, `InputAction`/`InputActionMessage` are live. Entity events (`entity.entered/exited/interacted`) live. |
| Planned event domains         |   ⛔   | (AI, dialogue, networking) are planned. Interaction events are now live. |
| Entity FSM (per-entity behavior) |   ✅   | `behavior` field on `PrefabDef`; `.behavior.ron` uses `StateMachineAsset` format; `{self}` substitution; `entity_fsm_interpreter_system` runs alongside global interpreters. |
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
| Player movement             |   ✅   | `MovementConfig` on prefab `components.movement`; works for both primitive and GLB players. Fields: `walk_speed`, `run_speed`, `rot_speed`, `jump`, `double_jump`, `collider_radius` (GLB), `collider_height` (GLB). Emits `player.jumped` trigger on every jump — bind sounds/effects in `state_machine.ron`. |
| Camera modes                |   ✅   | Unified `ActiveCameraMode` (`camera_modes.md` v1+v2) — `Orbit`/`Follow`/`FirstPerson`/`Fixed`/`Flycam`/`Party`, authored via `components.camera_mode` (or the legacy `camera:`/`flycam:` fields, still supported unchanged). Orbit is data-configured via `player.camera`; Flycam is spawned via the `"flycam"` tag on a prefab (LMB/RMB hold to look, WASD move, Shift fast mode, optional `flycam_position` UI label). Runtime mode-switching (v2) via `Action::SetCameraMode`, a scene-level `camera_modes:` named-preset registry, `owner_player` local-coop targeting, and eased `CameraBlendState` transitions. |
| Animation playback          |   ✅   | Data‑configured via `player.animations`. |
| NPC AI                      |   ✅   | `NpcAgent` component; states: Idle → Patrol → Alerted → Chase/Flee/Interact → Return. FOV + optional Rapier line-of-sight check. Emits `"npc.player_spotted:{id}"`, `"npc.player_reached:{id}"`, `"npc.player_lost:{id}"` triggers. |
| Collectible triggers        |   ✅   | `Collectable` component on Rapier sensor; on player overlap emits `GameEvent::Trigger("entity.collected:{spawn_id}")`. Response (Despawn, IncrementVariable, etc.) is configured in RON. |
| Trigger zones               |   ✅   | `TriggerZone` component + Rapier sensor; emits `entity.entered:{id}` / `entity.exited:{id}` on player enter/exit. Add via `trigger_zone` field on `PrefabDef`. |
| Interactable entities       |   ✅   | `Interactable { radius }` component; when player is within `radius` metres and presses the interact key (`inputs.interact` on the player prefab, default `"KeyF"`), emits `entity.interacted:{id}`. Add via `interactable` field on `PrefabDef`. |
| Motion (rotate/bob)         |   ✅   | `Motion` component; world-space continuous rotation (per-axis rad/s) and sinusoidal vertical bob (amplitude, frequency). Runs in `Update`; purely visual. |
| Particle effects            |   ✅   | `Action::SpawnEffect { key, position, entity }` bursts a named effect from `AssetCatalog.effects`. CPU pool renderer: one mesh entity per (blend_mode, texture) group — O(distinct textures) draw calls. `EffectDef` fields: count (≤256), lifetime, speed/jitter, spread_deg, offset, size/size_end/size_jitter, color_start/mid/end, gravity, turbulence, sprite/sprites (billboard quads), additive blend, uv_distort + uv_scroll_speed (PoolFlameMaterial animated flame), `layers` (multi-layer: one key fires several emitters at once). Demonstrated in `particles_demo` (campfire, torch row, magic shrine, smoke tower, explosion zone, frost crystal, healing fountain, star shower) and `effect_mayhem_demo` (stress test: 15 continuous emitters + MAYHEM burst button). |
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
| Platform parity tests |   ✅   | Headless Chromium browser test suite (`test_web.py`) covering all example projects. |

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
- `Action::Spawn { prefab, id, position, spawn_point, yaw_deg }` — enqueues a prefab spawn to `PendingEntitySpawns`; processed at max 2/frame by `drain_spawn_queue_system` to cap WebGPU pipeline-compile stalls on WASM; `id` auto-generated if omitted; `position` (x,y,z) takes precedence over `spawn_point` (scene-defined named point); defaults to world origin if neither is given; `yaw_deg` rotates around the Y axis
- `Action::PreloadPrefab(String)` — loads a prefab's GLB model early and stores the `Handle<Scene>` in `PreloadedGlbHandles` to prevent asset-server eviction; fire on `scene.ready` to eliminate the WASM GLB-decode stall on first spawn
- `Action::Despawn(String)` — removes a previously spawned entity by its spawn ID
- `Action::PlayAnimation(String)` — plays a named animation/clip
- `Action::PlaySound { key, volume }` — fire-and-forget sound by audio catalog key; `volume` (0.0–1.0, default 1.0) multiplies the per-entry catalog volume; warns for unsupported formats or missing keys
- `Action::PlayMusicLoop { key, volume }` — starts a looping background music track by audio catalog key; `volume` (0.0–1.0, default 1.0) multiplies the per-entry catalog volume
- `Action::StopMusic` — stops the current background music track
- `Action::SetVolume(u8)` — sets global volume 0–100
- `Action::PreloadScene(String)` — warms the asset cache for a `.scene.ron` before it is needed
- `Action::EnterState(String)` — transitions the interpreter to a named logic state; empty string returns to stateless (always-fire) default
- `Action::SetVariable(String, String)` — writes a named string value into `GameVariables`; readable by data-bound UI labels; `DebugState.score` is derived from the `"score"` key
- `Action::IncrementVariable(String, i32)` — parses the variable as `i32` and adds the delta; missing or unparseable values default to `0`
- `Action::PlayAnimationOn { target: String, clip: String }` — plays `clip` on the entity with the given spawn ID; use `"{self}"` as target inside `.behavior.ron` files
- `Action::EmitEvent(String)` — emits a `GameEvent::Trigger`; `{self}` in the string is substituted with the entity's spawn ID when used inside `.behavior.ron` files
- `Action::ShowDamagePopup { entity: String, amount: f32 }` — spawns a floating `+N` / `-N` world-space label above the entity identified by spawn ID; positive amounts show in heal colour, negative in damage colour; style (font size, duration, rise speed, colours) is configured via `damage_popup_style` in `.project.ron`; `{self}` is substituted in behavior files
- `Action::SetEntityVisible { entity: String, visible: bool }` — shows or hides a spawned entity by its spawn ID; the entity stays in the ECS (stats, colliders, and behavior FSM keep running); world labels (stat bar, stat label) tracking the entity auto-hide; `{self}` substituted in behavior files
- `Action::EmitEventAfterDelay { event: String, delay_secs: f32 }` — fires a `GameEvent::Trigger` after `delay_secs` seconds have elapsed; cleared on `Action::LoadScene` so events do not leak across scene transitions; `{self}` substituted in behavior files
- `Action::SpawnEffect { key: String, position: Option<(f32,f32,f32)>, entity: Option<String> }` — bursts a particle effect from `AssetCatalog.effects`; `entity` wins over `position`; `EffectDef.offset` added to origin; additive blending; `{self}` substituted in behavior files

### Entity messages (Beta 0.4)
- `entity.entered:{id}` — emitted by `TriggerZone` when the player enters the sensor collider
- `entity.exited:{id}` — emitted by `TriggerZone` when the player exits the sensor collider
- `entity.interacted:{id}` — emitted by `Interactable` when player is within `radius` metres and presses the interact key (`inputs.interact` on the player prefab, default `"KeyF"`)
- `stat.{id}.{stat_name}.depleted` — emitted by the stat threshold system when `stat_name` on the entity with spawn ID `id` reaches a `BelowOrEqual(0.0)` threshold (pattern from `stat_templates`; `{self}` in `emit` is resolved at spawn time)

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
- `rules_path: "logic/rules.ron"` in project config.
- Rules with `when` omitted fire in any logic state.
- Rules with `when: "state_name"` only fire while the interpreter is in that named state.
- `Action::EnterState(name)` transitions to a named state; `EnterState("")` returns to stateless.

### state_machine.ron workflow (schema v1) ✅
- `state_machine_path: "logic/state_machine.ron"` in project config.
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
      ( from: "playing", on: "ui.button_pressed:toggle_pause", to: "paused" ),
      ( from: "paused",  on: "ui.button_pressed:toggle_pause", to: "playing" ),
    ],
  )
  ```

## UI v1 Scope (authoring)
- Supported elements: `Button((...))`, `Label((...))`, `Rect((...))`, `StatBar((...))`, `StatSpread((...))` — typed RON enum variants; unknown variants are rejected at parse time.
- Example:
  ```ron
  ui: [
    Button((
      id: "start_button",
      text: "Start Game",
      action: "ui.start_game",
      position: (100.0, 100.0),
      size: (300.0, 65.0),
    )),
    Label((
      id: "score_label",
      text: "Score  0",
      bind: "score",
      format: "Score  {}",
      position: (16.0, 16.0),
      size: (180.0, 32.0),
    )),
    // Stat bar auto-fills from LoadedStats[stat_key] — no event wiring needed.
    StatBar((
      id: "health_bar",
      stat_key: "player_health",
      position: (16.0, 56.0),
      size: (200.0, 18.0),
      fill_color: (0.85, 0.15, 0.15, 1.0),
      background_color: (0.20, 0.06, 0.06, 1.0),
      show_value: true,
      color_bands: [
        ( above_percent: 0.5,  color: (0.85, 0.15, 0.15, 1.0) ),
        ( above_percent: 0.25, color: (1.0,  0.55, 0.0,  1.0) ),
        ( above_percent: 0.0,  color: (0.6,  0.0,  0.0,  1.0) ),
      ],
      absolute: true,
    )),
    // Stat spread lists multiple stats as labelled minibar rows.
    StatSpread((
      id: "stat_panel",
      stats: ["player_health", "player_mana", "player_stamina"],
      position: (16.0, 84.0),
      label_width: 110.0,
      bar_width: 150.0,
      row_height: 22.0,
      show_values: true,
      absolute: true,
    )),
  ]
  ```

---
name: local-coop-pattern
description: Local co-op Stages 1+3 — PartyOrbitCamera, split-screen viewport, gamepad routing, view-box clamp; four player-construction sites, player_index dead-field footgun, split-vs-party per-player-config asymmetry
metadata:
  type: project
---

**STAGE 3 — vertical split-screen (reviewed 2026-07-05, ALIGNED, uncommitted at review time):**
- `CameraConfig.split: Option<SplitScreenDef{orientation: SplitOrientation}>` (schema/player.rs).
  `SplitOrientation` enum has ONLY `Vertical` today (`Horizontal`/`Dynamic` reserved for Stages 4-5,
  intentionally NOT added yet — a `deny_unknown_fields`-free enum, so a RON `Horizontal` would be a
  clean parse error, not silent). Fully `#[derive(Deserialize)]`, `#[serde(default)]` on `split`.
- `split_screen_viewport_system` (capabilities/camera.rs) touches ONLY `Camera.viewport`; reads
  `ActiveSplitScreen` resource + primary `Window::physical_size()`. NO ActionQueue, no gameplay
  reach-over. Correct. Runs in Update alongside camera_orbit/party_camera_follow (lib.rs).
- `ActiveSplitScreen(Option<SplitOrientation>)` resource (scene_manager/mod.rs) — mirrors ActiveViewBox
  EXACTLY: init_resource in lib.rs, set by spawn_players_and_camera, cleared on Action::LoadScene
  (action_executor.rs). Orientation kept OFF the SplitViewportSlot component deliberately (camera_modes
  unification hygiene).
- `parse_orbit_button` "None" arm → `(false, false)`, NO warning (distinct from unknown string, which
  warns+defaults "Either"). Genuine designer opt-out.
- split+party both set: `spawn_players_and_camera` warns and `split` wins (entity_spawner.rs ~540).
  Pure warn+precedence, no demo-specific hardcoding. Data-driven.

**CRITICAL ASYMMETRY (footgun to flag on any co-op camera review):** `party` reads ONLY the first
player's full camera block; later players' camera fields are ignored. `split` is the OPPOSITE — it
spawns one real OrbitCamera PER player from THAT player's OWN camera block (only `split`/`party`
themselves are first-player-only). So split-screen requires `zoom_speed: 0.0` + `orbit_button: "None"`
on EVERY player's camera block, not just the first — omit it on player 2 and a shared mouse
orbits/zooms both cameras together. local_coop_demo's prefabs.ron documents this inline on both
player_p1_split (196/200) and player_p2_split (238/242). If reviewing a new split scene, verify every
player prefab has these two knobs, not just player 1.

**Known accepted split-screen limitations (documented in src/CLAUDE.md, not bugs):** CameraShake now
fires on BOTH split cameras (real OrbitCameras); world_label/nameplate/particle_renderer/targeting all
assume one Camera3d and silently pick/no-op one — none affect local_coop_demo (uses none of them).

Local co-op (same-machine 2-player) extends the player-spawn pipeline. See
[[player_spawn_via_action_pattern]] for the base Action::Spawn player promotion.

**Designer reachability — all genuinely RON-authorable:**
- `PrefabDef.player_index: u32` (schema/catalog.rs) — authored per player prefab.
- `PlayerConfig.player_index` (schema/player.rs) — forwarded via `assemble_player_config`.
- `InputMap.gamepad_index: Option<usize>` (schema/player.rs) — authored in `components.inputs`;
  `input_translator_system` (runtime/input.rs) sorts connected `Gamepad` entities by
  `entity.index()` and picks the nth. NOT a hardware slot — "nth connected this session".
- `CameraConfig.party: Option<PartyZoomDef{zoom_margin, allow_manual_zoom}>` — the SOLE explicit
  switch for the shared camera. Read from the FIRST player-tagged scene entity only; later
  players' `camera`/`party` fields are ignored. Scene `entities:` order matters for co-op.
- `GameSceneV2.max_view_box: Option<(min_x,min_z,max_x,max_z)>` → `ActiveViewBox` resource
  (mod.rs), cleared on LoadScene (action_executor.rs), set on scene load (scene_loader.rs).

**No ActionQueue push, no gameplay reach-over (both correct):**
- `party_camera_follow_system` (capabilities/camera.rs) mutates only camera Transform +
  PartyOrbitCamera state; reads target Transforms. In Update, chained after camera_orbit_system
  (mirrors the OrbitCamera sibling — both live in Update, NOT FixedUpdate as the plan mis-stated).
- `player_view_box_clamp_system` (capabilities/player.rs) mutates only Transform + Velocity;
  FixedUpdate, chained after player_movement_system. Correct.

**FOOTGUN — `player_index` is a DEAD FIELD (as of 2026-07-04 impl).** It is threaded schema →
PlayerConfig → stored, but has ZERO runtime consumers. Input routing keys off `gamepad_index`
(InputMap), camera targeting keys off the `entities` Vec order — neither reads `player_index`.
It is NOT inserted as a component on the player entity, so no future system can query it either.
Grep `player_index` in src/: only appears at catalog.rs (def), player.rs (def), entity_spawner.rs:784
(the assemble copy). Harmless (default 0, serde default) but it's dead weight that gives a false
impression of designer control — a designer setting `player_index: 1` sees no effect. Stage 2+
(portal moving "both players") will likely need it; until a consumer exists it's speculative
schema. If reviewing a change that claims to "use" player_index, verify a real query/insert lands.

**Four player-construction sites (from src/CLAUDE.md — keep in sync):**
1. GLB collector — scene_loader.rs builds `player_configs: Vec<PlayerConfig>`.
2. Primitive/capsule inline — single-player ONLY; co-op does not extend to it (documented scope).
3. Dynamic Action::Spawn — action_executor.rs, one player, `assemble_player_config`.
4. Shared spawn fns — entity_spawner.rs `spawn_player_entity` (1 player, own OrbitCamera) +
   `spawn_players_and_camera` (2+ share one PartyOrbitCamera). Both call `spawn_player_entity_core`.
Sites 1+3 route through `assemble_player_config` — new PlayerConfig fields go there once.

**Known accepted limitation:** Action::CameraShake queries only `With<OrbitCamera>`
(SceneStateParams::orbit_cameras), so it no-ops on PartyOrbitCamera scenes. Documented, out of
Stage 1 scope.

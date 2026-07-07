---
name: local-coop-pattern
description: Local co-op Stages 1+3 — PartyOrbitCamera, split-screen viewport, gamepad routing, view-box clamp; four player-construction sites, player_index dead-field footgun, split-vs-party per-player-config asymmetry
metadata:
  type: project
---

**STAGE 6 — 4-way / N-way Grid split-screen (reviewed 2026-07-07, ALIGNED):**
- `SplitOrientation::Grid` variant (additive; Vertical/Horizontal unchanged, still 2-way). Reachable
  via `split: (orientation: Grid)` on FIRST player's camera block only (same first-player-wins rule).
- Player count is NOT authored directly — derived from scene `entities:` count (tags:["player"]),
  capped at `MAX_SPLIT_PLAYERS: u32 = 4` (camera.rs:247). Grid math is generic (cols=ceil(sqrt(count)),
  rows=ceil(count/cols), row-major cell by slot.0) but the cap is a hardcoded perf/safety ceiling
  (bounds WebGPU render-pass count) — a designer wanting 5+ way silently gets 4 cams + extras
  cameraless. NOT RON-tunable; accepted (documented, analogous to SPAWNS_PER_FRAME), NOT a blocker.
- Quadrant assignment = entity order in `entities:` (slot 0-3 = spawn order). Designer-controllable
  but MUST be known — documented in room6.scene.ron comment + src/CLAUDE.md. count==3 leaves one
  dead (clear-color) quadrant, documented.
- `ActiveSplitSlotCount(Option<u32>)` resource (mod.rs:162) mirrors DynamicSplitConfig EXACTLY:
  init lib.rs:158, set at all entity_spawner branches (Some(slot_count) only in Grid, None else),
  cleared LoadScene (action_executor.rs:56). Deliberately stored-not-live-queried so future hot-
  join/leave won't reflow grid on mid-transition churn. No hidden non-RON behavior.
- `split_screen_viewport_system` Grid arm (camera.rs:315) touches ONLY Camera.viewport; slot.0 >=
  cols*rows → skip (unpositioned, not bogus cell). NO ActionQueue. Correct.
- Numpad0-9 added to InputMap::parse_key (player.rs:283) — additive whitelist, RON-authored.
- Visual distinction = ZERO new capability: 4 `tint_*` Standard materials in assets.ron + per-prefab
  `material: "tint_blue"` catalog-key override (PrefabDef.material → PendingMaterialOverride). No
  hardcoded paths. player_p1-4_grid each repeat zoom_speed:0.0 + orbit_button:"None" per the
  per-player-config asymmetry footgun below (correct). Grid does NOT support split.dynamic (2-way only).

**STAGE 5 — dynamic split-screen (reviewed 2026-07-06, ALIGNED):**
- `SplitScreenDef.dynamic: Option<DynamicSplitDef>`; `orientation` now `#[serde(default)]` (Vertical),
  in dynamic mode it's only a rare-tie-break hint (live axis chosen from dx/dz). `DynamicSplitDef`
  fields ALL RON-authored: `split_distance`/`merge_distance` (no default, per-scene), `merged_zoom_margin`,
  `merged_allow_manual_zoom` (default false). No magic constants leaked to Rust — only the anti-flicker
  clamp `split_distance - 0.01` is a code literal, which is correct (it's a jitter epsilon, not a tunable).
- Dynamic spawns ALL 3 cameras up front (party + 2 split), toggles only `Camera.is_active`; neither
  camera_orbit_system nor party_camera_follow_system gate on is_active so inactive cams stay Transform-
  correct (no pop on reactivate). `DynamicSplitConfig(Option<DynamicSplitDef>)` resource: init lib.rs:157,
  set at all 4 entity_spawner branches (Some only in dynamic branch, None elsewhere), cleared on LoadScene
  (action_executor.rs:55). Mirrors ActiveSplitScreen exactly.
- `dynamic_split_screen_system` (camera.rs:331) touches ONLY Camera.is_active + ActiveSplitScreen; reads
  DynamicSplitConfig + split cams' tracked-player Transforms. NO ActionQueue, no gameplay side effects.
  Hysteresis correct: currently_split uses merge_distance, else split_distance; early-return when no state
  change. Chained in Update after party_camera_follow, before split_screen_viewport (lib.rs:293).
- merge<split validation is proper warn-and-clamp (entity_spawner.rs:556): if merge>=split, warn + clamp
  merge to split-0.01. No panic, no silent misbehavior.
- No hardcoded asset paths in room5.scene.ron / prefabs.ron / rules.ron — models are catalog keys
  (character_male/female), LoadScene uses scene-file refs (engine convention). player_p2_dynamic correctly
  repeats zoom_speed:0.0 + orbit_button:"None" per the per-player-config asymmetry footgun below.

**STAGES 3+4 — vertical + horizontal split-screen (reviewed 2026-07-05, both ALIGNED):**
- `CameraConfig.split: Option<SplitScreenDef{orientation: SplitOrientation}>` (schema/player.rs).
  `SplitOrientation` enum has `Vertical` AND `Horizontal` implemented today (Stage 4 added
  `Horizontal`, player.rs:118). `Dynamic` still reserved for a later stage — NOT added yet, so a RON
  `Dynamic` is a clean parse error, not silent. Fully `#[derive(Deserialize)]`, `#[serde(default)]`
  on `split`.
- `split_screen_viewport_system` (capabilities/camera.rs:273) touches ONLY `Camera.viewport`; reads
  `ActiveSplitScreen` resource + primary `Window::physical_size()`. NO ActionQueue, no gameplay
  reach-over. Correct. Runs in Update alongside camera_orbit/party_camera_follow (lib.rs).
  Both match arms are pure viewport geometry: Vertical splits `physical_width` L/R, Horizontal splits
  `physical_height` T/B (slot 0 = top, y=0). Slot-1 half uses `full - half` remainder to absorb
  odd-pixel rounding so the two halves always sum exactly. Zero per-project/hardcoded logic — any
  designer's own project gets working H-split from `split: (orientation: Horizontal)` alone.
- Stage 4 demo: local_coop_demo room4 (scenes/room4.scene.ron) + player_p1_split_h/player_p2_split_h
  (prefabs.ron:298/337) + ground_room4/portal_to_room4 + rules.ron portal wiring. No new
  resources/components; entity_spawner spawn logic was already orientation-agnostic.
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

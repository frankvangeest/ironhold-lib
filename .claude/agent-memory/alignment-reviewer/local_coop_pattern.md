---
name: local-coop-pattern
description: Local co-op Stage 1 — PartyOrbitCamera, gamepad routing, view-box clamp; the four player-construction sites and the player_index dead-field footgun
metadata:
  type: project
---

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

---
name: camera-modes-v1-pattern
description: camera_modes v1 (ActiveCameraMode/CameraTargets/per-mode markers) — the camera-spawn-site fan-out that makes new CameraModeDef variants easy to leave half-wired; 3 unregistered systems + Orbit-payload-ignored-on-split + FOV 45->60 default break found at review
metadata:
  type: project
---

Reviewed 2026-08-07 (v1, verdict BLOCKING). Replaces `OrbitCamera`/`PartyOrbitCamera`/`FlyCamera`
with `ActiveCameraMode` (enum component, runtime state) + `CameraTargets(Vec<Entity>)` (ownership,
present on every camera) + zero-sized per-mode markers (`OrbitCameraMode`/`PartyCameraMode`/
`FixedCameraMode`/`FollowCameraMode`/`FirstPersonCameraMode`/`FlycamCameraMode`) so Bevy queries can
filter by kind. Authored schema is `CameraModeDef` in `schema/camera.rs`
(`components.camera_mode`), with `split:`/`party:` promoted to siblings of `camera_mode` under
`components:` (`PrefabComponents::split`/`::party`), dual-sourced in `assemble_player_config`
(sibling first, legacy `camera.split`/`camera.party` fallback). RON gotcha: newtype-variant-wrapping-
struct needs double parens — `Orbit((offset: ...))`.

**THE STRUCTURAL FOOTGUN — there are 6+ camera-spawn sites and only ONE dispatches on
`camera_mode`.** `spawn_active_camera_for_player` (entity_spawner.rs) is the mode-generic one, and
it is reached ONLY from the single-player paths (`spawn_player_entity`, and
`spawn_players_and_camera`'s `entities.len() < 2` branch). Every co-op path —
`spawn_split_camera_for_player`, the dynamic-split 2-cam loop, the hot-join site
(entity_spawner.rs:367), the party camera (`spawn_party_orbit_camera`), and the "2+ players but no
split/party" warn-fallback — calls `spawn_orbit_camera_for_player`/`spawn_party_orbit_camera`,
which read the **legacy `PlayerConfig.camera`** field and never consult `camera_mode`. Since a
prefab migrated to `camera_mode: Orbit((...))` typically drops its `camera:` block entirely,
`PlayerConfig.camera` silently becomes `default_camera_config()` (orbit_button "Either",
zoom_speed 10.0, max_radius 20). Concretely this regressed the shipped v1 migration proof
(`local_coop_demo` room4 `player_p1_split_h`/`player_p2_split_h`), reintroducing the classic
"one shared mouse orbits/zooms both split cameras" bug. **Any future camera work: grep
`spawn_orbit_camera_for_player` + `spawn_party_orbit_camera` call sites and confirm each resolves
`resolve_camera_mode` first.**

**Other v1 findings (verify whether fixed before trusting the feature):**
- `fixed_camera_system`/`follow_camera_system`/`first_person_camera_system` were written but never
  `add_systems`-registered in `lib.rs` — `Follow`/`FirstPerson`/`Fixed` parsed and spawned but were
  frozen no-ops while documented as working. lib.rs's camera `.chain()` (camera_orbit_system →
  party_camera_follow → dynamic_split → split_viewport → labels → target_hud → camera_shake →
  fly_camera) is the registration site.
- `CameraConfig.fov` (new, `default_fov() = 60.0`) is inserted as an explicit
  `Projection::Perspective` on every orbit-family camera via `insert_fov`. Bevy 0.18's
  `PerspectiveProjection::default()` is **45°** (PI/4), so this silently widened FOV on every
  existing project with zero RON change — and party/flycam/default cameras get no `insert_fov`, so
  a `split.dynamic` scene visibly pops 45°↔60° on merge/split. Any `fov` default must be 45.0 (or
  `insert_fov` must apply to all camera kinds) to keep existing RON unchanged.
- `PartyCameraDef` (8 fields) is fully dead schema — `CameraModeDef::Party(_)` only warns and falls
  back to Orbit; nothing reads the payload. Documented as intentional ("for completeness").
- `camera_mode:` is read ONLY for `tags: ["player"]` (via `assemble_player_config`) and
  `tags: ["flycam"]` (scene_loader.rs ~236). On any other prefab it parses and is silently
  ignored — no warn. So a player-less `Fixed` cutscene/menu camera is not authorable in v1.
- `FixedCameraDef` allows both `look_at` and `look_at_entity` to be `None` (identity rotation, no
  warn); `look_at_entity` is not CLI-validated against scene entity ids.
- CLI has **no** camera validation at all (`validate.rs` never touches `camera`/`orbit_button`/
  `split`), so the whole camera surface is runtime-warn-only.
- Correct by construction: no new system touches `ActionQueue`; `CameraShake`'s
  `SceneStateParams::orbit_cameras` is now `Or<(With<OrbitCameraMode>, With<PartyCameraMode>)>`,
  closing the old party-scene gap while keeping the flycam `warn!`; no hardcoded asset paths; no
  RNG (WASM-safe).

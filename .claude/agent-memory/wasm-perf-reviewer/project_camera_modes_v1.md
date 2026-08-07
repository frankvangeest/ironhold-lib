---
name: camera-modes-v1
description: ActiveCameraMode enum + per-mode marker components replaced OrbitCamera/PartyOrbitCamera/FlyCamera — marker-filtered queries keep per-frame cost identical; FOV default silently changed 45deg->60deg
metadata:
  type: project
---

`capabilities/camera.rs` now holds one `ActiveCameraMode` enum component (6 variants: Orbit/Party/
Fixed/Follow/FirstPerson/Flycam) plus 6 zero-sized marker components and `CameraTargets(Vec<Entity>)`.
Authored schema lives in `schema/camera.rs` (`CameraModeDef`), resolved at spawn by
`entity_spawner::resolve_camera_mode` / `spawn_active_camera_for_player`.

**Why the enum costs nothing per frame:** every camera system pairs `Query<&mut ActiveCameraMode, ...>`
with the matching `With<XCameraMode>` marker filter, so Bevy narrows at the archetype level and the
`let ActiveCameraMode::X(s) = &mut *mode else { continue }` inside the loop is a single discriminant
compare on entities that already matched. There is NO bare `Query<&mut ActiveCameraMode>` anywhere.
If a future mode/system is added, keep the marker filter — dropping it is the only way this pattern
becomes a real cost.

**Change-detection footgun introduced here:** `&mut *mode` is a `DerefMut`, so `ActiveCameraMode` is
marked Changed *every frame* on Orbit/Party/FirstPerson/Flycam cameras even on fully idle frames.
Harmless today (nothing filters `Changed<ActiveCameraMode>`), but it means such a filter can never
work. `fixed_camera_system`/`follow_camera_system` correctly use `&ActiveCameraMode` instead.

**FOV / `Projection`:** `CameraConfig.fov` defaults to **60 degrees**, but Bevy 0.18's
`PerspectiveProjection::default()` is **45 degrees** (PI/4) and `Camera3d` has `#[require(Camera, Projection)]`.
`insert_fov` (entity_spawner.rs) overrides it via a follow-up `commands.entity(e).insert(...)` — safe
(same command flush, Projection already in the archetype, no archetype move, `update_frusta` runs in
PostUpdate) but it widens FOV on every pre-existing scene and therefore invalidates every
`screenshot_baselines/scenes/*.png`. `spawn_party_orbit_camera` does NOT call `insert_fov`, so party
cameras stay at 45deg while split cameras get 60deg — visible FOV pop on `split.dynamic` merge/split.

**Split/party camera spawn does NOT go through `resolve_camera_mode`.** `spawn_split_camera_for_player`
and the `party:` branch both read `player_config.camera` (the legacy field) directly. A prefab that
authors only `camera_mode: Orbit((...))` gets `default_camera_config()` for those paths — silently
reverting `orbit_button: "None"`, `zoom_speed: 0.0`, offsets, etc.

Related: [[project_camera_shake_system]], [[project_dynamic_split_screen]], [[project_split_screen_viewport]], [[project_wasm_size]].

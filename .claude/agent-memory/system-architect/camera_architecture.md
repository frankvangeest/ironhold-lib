---
name: camera-architecture
description: How OrbitCamera and FlyCamera coexist as siloed camera components, the Update-schedule camera chain, and the camera_modes unification plan that will replace them
metadata:
  type: project
---

The engine has **two siloed camera components**, `OrbitCamera` and `FlyCamera` (both `capabilities/`), selected at scene-load time by entity tags (the `"flycam"` tag spawns a `FlyCamera`). They cannot switch mid-session.

**Camera systems run in Update, not FixedUpdate** — deliberate, because camera is render-cadence not physics. The Update chain (lib.rs ~line 230) is: `animation_resolver_system` → `camera_orbit_system` → `camera_shake_system` → `fly_camera_system` → `animation_playback_system`, all `.chain()`ed. `camera_orbit_system` writes the orbital `cam_transform.translation` each frame from yaw/pitch/radius; any system that wants to perturb the camera (e.g. shake) MUST run AFTER it in the same chain and apply an **additive** offset (`+=`), or the orbit system overwrites it next frame. This is why `camera_shake_system` is chained immediately after `camera_orbit_system`.

Note the core CLAUDE.md rule "physics & camera-follow logic must run in FixedUpdate" refers to the *character-follow* coupling; the actual orbit/flycam camera systems live in Update. Don't flag a camera system in Update as a schedule violation.

**Shake targets OrbitCamera only** (`With<OrbitCamera>` filter on both the executor query and `camera_shake_system`). FlyCamera scenes (terrain_demo, custom_materials) get a logged no-op warning — correct and intentional. There is no shake for flycam; a flycam additive offset would fight `fly_camera_system` which also writes translation each frame.

**The camera_modes feature (planning/features/camera_modes.md) plans to replace BOTH `OrbitCamera` and `FlyCamera` with a single `ActiveCameraMode` component + unified `camera_system` dispatching on mode, plus `Action::SetCameraMode`.** Any new per-camera component (like `CameraShakeState`) adds migration surface to that refactor. When reviewing new camera features, note whether they will need re-homing onto `ActiveCameraMode` — flag the coupling so the camera_modes work accounts for it.

**`OrbitCamera.yaw`/`.pitch` are written by exactly ONE system: `camera_orbit_system`** (verified 2026-07-19). Nothing else reads or mutates them (flycam and `PartyOrbitCamera` have their own separate yaw/pitch). So making them additionally keyboard-writable (per-player keyboard look feature) affects no other consumer — the only frame ordering that matters is shake-after-orbit, already handled.

**Per-player keyboard look SHIPPED as designed (feature/camera-look-controls, 2026-07-19):** `OrbitCamera` gained `look_left_key`/`look_right_key`/`look_up_key`/`look_down_key: Option<KeyCode>` + `look_speed: f32`, all pre-resolved once at spawn from `InputMap.look_*` strings via `InputMap::parse_key` (same precedent as `orbit_lmb`/`orbit_rmb`). `camera_orbit_system` got a `Res<ButtonInput<KeyCode>>` param and a keyboard block that runs UNCONDITIONALLY (independent of the mouse `orbit_active` gate) — the point being split-screen sets `orbit_button:"None"`. Pitch convention is now PINNED in code + test: `look_up` increases pitch toward `max_pitch` (overhead), matching the mouse convention, not "up = sky". `look_speed` is a deliberately separate dial from `orbit_speed` (rad/s hold-rate vs mouse-pixel-delta multiplier) and is forward-designed to also drive gamepad right-stick pitch. `PartyOrbitCamera` was correctly left out (no single per-player owner for a binding).

**Pitch-direction trap:** `CameraConfig.min_pitch` (default 0.1) is documented "looking up", `max_pitch` (default 0.9) "looking down" — the whole authored range is downward-ish angles, there is no true look-at-sky. Higher pitch = camera positioned higher = more top-down (verified from the `Quat::from_axis_angle(X, -pitch)` math). The existing mouse convention: mouse-up (negative screen delta.y) → `pitch += ` → more top-down. Any new "look up"/"look down" binding must pick a convention deliberately: matching the mouse means look_up → pitch increase → overhead, but a player may intuit "look up" as raising the aim toward the horizon = pitch DECREASE. Pin the convention in the feature and assert direction (not just clamp bounds) in tests.

**The camera spawn helpers are at their positional-parameter limit.** `spawn_split_camera_for_player`
(entity_spawner.rs) and `spawn_party_orbit_camera` (camera.rs) are both at 6 positional params after
`per_viewport_target_ring_visibility` added an `own_viewport_only: bool` to each. Both are still
legible (the bool is last, no adjacent bool to transpose with) and this codebase has no
options-struct precedent for spawn helpers — but the *next* per-camera toggle should introduce a
small `SplitCameraOpts`/`PartyCameraOpts` struct rather than a 7th positional param. The
`camera_modes` refactor re-homes both helpers anyway, so fold it in there.

**`OrbitCamera` is constructed at exactly two sites** (both must be updated for any new field, non-`Option` fields also break `default_camera_config()` in entity_spawner.rs and the `base_camera_config()`/test literals since neither `CameraConfig` nor `InputMap` derive `Default`): `entity_spawner.rs::spawn_orbit_camera_for_player` (GLB path, incl. all split-screen per-player cameras) and the primitive/capsule inline block in `scene_loader.rs` (single-player only — local co-op split-screen never uses it). Both have the full `PlayerConfig`/`components` in scope, so both `.camera` and `.inputs` (InputMap) are reachable — splitting look config across the two structs (keybinds in InputMap, speed in CameraConfig) is wireable at both.

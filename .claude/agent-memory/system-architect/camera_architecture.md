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

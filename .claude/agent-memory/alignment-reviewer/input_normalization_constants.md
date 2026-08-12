---
name: input-normalization-constants
description: Where the RON-knob-vs-engine-constant line falls for raw input device normalization (scroll wheel, mouse motion) in capabilities/camera.rs, plus the dt-mixing inconsistency across the 5 camera mode systems
metadata:
  type: project
---

Reviewed 2026-08-10 (`feature/camera_zoom_smoothing`, verdict ALIGNED). Precedent established:
**raw-device normalization factors are engine constants, not RON fields** — `SCROLL_PIXELS_PER_LINE`
(`capabilities/camera.rs`, 20.0) and the `MouseScrollUnit::Line` ±1.0 per-event clamp inside
`normalized_wheel_delta` are NOT hardcoding blockers, because the designer's authored knob
(`CameraConfig.zoom_speed` / `OrbitCameraDef.zoom_speed` / `PartyCameraDef.zoom_speed`) multiplies
the normalized value and so retains the full expressive range. The reasoning to reuse: a designer
cannot know the end user's OS scroll setting/device, so exposing the divisor in RON would push a
per-machine value into per-project data. **Corollary risk:** because `zoom_speed` is one
platform-shared value, any *mis*-calibration of the normalizer is un-fixable from RON — so
per-`MouseScrollUnit` calibration accuracy (especially browser `Pixel` deltas, which are what the
WASM build actually receives — Chrome ~100px/notch vs Firefox `Line` units) is an engine
correctness obligation, not a designer tuning problem.

**Only two `MouseWheel` readers exist in the whole workspace** (`camera_orbit_system`,
`party_camera_follow_system`, both in `capabilities/camera.rs`) — nothing in `inventory.rs`,
`inspector.rs`, `flycam.rs`, `ironhold_web`, or the UI layer consumes scroll. Any "did the fix
cover every scroll path?" question is answered by grepping `MouseWheel` in `crates/`.

**Standing inconsistency to expect on any camera-input change:** `camera_orbit_system` and
`party_camera_follow_system` multiply *accumulated relative* mouse-motion deltas by
`time.delta_secs()` (frame-rate-dependent sensitivity, double-counts on slow frames), while
`first_person_camera_system` and `flycam.rs`'s `fly_camera_system` correctly use a bare
`sensitivity` factor with no dt. The wheel path also dt-scales a *discrete impulse*
(`zoom_delta * zoom_speed * dt`), so a frame hitch (Bevy's `Time<Virtual>` `max_delta` is 250ms)
still yields ~15x the normal zoom step. Removing the dt would change the authored meaning of
`zoom_speed`/`orbit_speed` in every shipped project, so it needs a plan + migration pass, not a
silent fix.

**Runtime zoom tuning has no dedicated Action:** `zoom_speed` is baked into `OrbitState` at spawn.
The only RON path to change it live is a `camera_modes:` preset + `Action::SetCameraMode`, and
`apply_camera_mode`'s Orbit arm calls `orbit_state_from_config`, which rebuilds state fresh —
radius/yaw/pitch reset, so the camera visibly snaps. Relevant to backlog's requested
"zoom-speed option in `3rd_person_game_demo`'s options menu". See [[camera_modes_v2_pattern]].

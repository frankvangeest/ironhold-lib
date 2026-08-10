---
name: camera-mode-switch-spawn-only-state
description: Runtime camera-mode switching (Action::SetCameraMode) breaks on any mode whose state is applied once at spawn and never recomputed per frame — Fixed's position, Flycam's yaw/pitch, CameraTargets
metadata:
  type: project
---

Per-mode camera systems in `capabilities/camera.rs` split into two classes, and a runtime
mode-switch path (`apply_camera_mode` in `runtime/scene_manager/entity_spawner.rs`) is only correct
for one of them:

- **Recomputed every frame** (Orbit, Follow, FirstPerson, Party) — the system derives `Transform`
  from `CameraTargets` each tick, so a marker/`ActiveCameraMode` swap is sufficient.
- **Applied once at spawn** — `FixedCameraDef.position` is written straight into the spawn
  `Transform` and does not exist on `FixedState` at all (`fixed_camera_system` only calls
  `look_at`, never touches translation); `FlycamState.pitch/yaw` are computed from the authored
  transform at spawn in `scene_loader.rs` but `fly_camera_system` writes
  `rotation = from_euler(yaw, pitch)` unconditionally; `CameraTargets` is fixed at spawn and empty
  for the Fixed/Flycam arms.

**Why:** a mode switch that only swaps the marker + `ActiveCameraMode` silently drops every
spawn-only field. Found in the camera_modes v2 review (2026-08-09): `SetCameraMode` onto a
`Fixed(position: ...)` registry preset left the camera wherever it was and merely rotated it —
both shipped demo presets were `Fixed`, and no test asserted translation.

**How to apply:** whenever a feature makes camera state switchable/reassignable at runtime, check
each `CameraModeDef` variant for fields consumed only at the spawn site, not by the per-frame
system. The same audit applies to `AuthoredCameraMode` coverage — a camera spawn site that forgets
it (e.g. `scene_loader.rs`'s scene-level Flycam) drops out of the `all_cameras` query entirely and
the action no-ops with no warning at all.

Related: [[project_renderlayers_reserved_scheme]] (same "spawn-time-only insertion" fragility class),
[[project_camera_mode_dual_source]].

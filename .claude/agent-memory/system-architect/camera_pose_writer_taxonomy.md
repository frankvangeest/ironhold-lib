---
name: camera-pose-writer-taxonomy
description: The absolute-pose vs accumulator-pose split across the six camera mode systems — the precondition that governs every render-pose perturbation system (shake, blend) and every runtime mode switch
metadata:
  type: project
---

**The single most load-bearing fact about the camera modes: the six per-mode systems are NOT
interchangeable in how they write `Transform`.** Two disjoint groups, and almost every camera bug
found in v1/v2 review traces back to code that assumed one group's behavior applied to all six.
Verified by reading every writer (`capabilities/camera.rs` 143/356/431/463/508/1049,
`capabilities/flycam.rs:61`) during the camera_modes v2 review, 2026-08-09.

**Group A — absolute-pose (pure function of world state, rewrites `Transform` from scratch each frame):**
`camera_orbit_system`, `party_camera_follow_system`, `first_person_camera_system`
(`transform.translation = target.translation + eye_offset`).

**Group B — accumulator (reads its own previous `Transform` as state):**
- `follow_camera_system` — `transform.translation.lerp(desired_pos, t)`; the Transform *is* the
  smoothing accumulator.
- `fly_camera_system` — the Transform *is* the position state; only input deltas are applied.
- `fixed_camera_system` — writes **rotation only**, never translation. `FixedState` has no
  `position` field at all; the authored `FixedCameraDef.position` is applied exactly once, at spawn,
  as `Transform::from_translation(...)` in `spawn_active_camera_for_player`.

**Why this matters, three concrete consequences:**

1. **Any system that perturbs the rendered pose must be filtered to Group A, or it corrupts Group
   B's state.** This is *already* why `camera_shake_system` carries
   `Or<(With<OrbitCameraMode>, With<PartyCameraMode>)>` — the "shake can never work on a flycam"
   limitation is this rule, not an accident. `camera_blend_system` (v2, Design A: blend the rendered
   pose toward whatever the live mode computed this frame, running last in the chain) has the same
   shape but no such filter, so `transition:` on `Follow`/`Flycam` is degenerate-to-harmful.
   The correct general fix is a save/restore pair — stash the un-blended live pose, restore it at
   the top of the chain before the mode systems read it — which would also unblock flycam shake.

2. **Any switch-time code that only swaps `ActiveCameraMode` + marker is incomplete for Group B.**
   Group A modes recompute their pose the next frame so the switch "just works"; Group B modes need
   the pose handed to them. `Fixed` is the worst case — switching to it leaves the camera wherever
   it was and merely rotates it. Cleanest structural fix: move `position` into `FixedState` and have
   `fixed_camera_system` write translation each frame, promoting `Fixed` into Group A.

3. **Marker removal on a mode switch silently drops the camera out of every marker-filtered query.**
   `dynamic_split_screen_system` (`With<OrbitCameraMode>, With<SplitViewportSlot>`) bails at its
   `let Some(t1) = targets.next() else { return }` when a split camera stops being Orbit — freezing
   the *whole* system, not just that camera. `camera_shake_system` likewise silently orphans a live
   `CameraShakeState` (never ticked, never removed, resumes if the camera returns to Orbit). Any
   marker-filtered camera query is now a runtime-mutable set, not a spawn-time-fixed one.

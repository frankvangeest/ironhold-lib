---
name: flycam-spawn-roll-until-first-write
description: A flycam spawns with the scene's XYZ-euler rotation (roll included) while FlycamState keeps only YXZ yaw/pitch; only fly_camera_system's first write drops the roll, so any gate before that write (sim hold, static mode, inspector) shows a rolled horizon
metadata:
  type: project
---

`scene_loader.rs` builds every entity rotation with `Quat::from_euler(EulerRot::XYZ, ..)`, and the flycam spawn stores
`to_euler(EulerRot::YXZ)` yaw/pitch in `FlycamState`, **throwing the roll away**. The spawned `Transform` still has the roll.
`fly_camera_system` overwrites `transform.rotation = from_euler(YXZ, yaw, pitch, 0)` every frame, so live play shows a
roll-free view after one frame. Anything that returns early before that write keeps the rolled spawn rotation: the
`!sim.is_running()` early return is the one that matters now.

Concrete case (2026-10-23, `feature/static_scene_mode` review): `terrain_demo` authors `rotation_euler_deg: (-30, 45, 0)`,
which is yaw 49.1 / pitch -20.7 / **roll -22.2 deg** in YXZ. The `?static=1` baseline (also the index.html gallery image)
shows a 22 deg tilted horizon no player ever sees. Pure-pitch flycams (`(-15,0,0)`, `(-18,0,0)`) are unaffected.

**Why:** spawn-only state that a per-frame system "corrects" on frame 1 hides spawn bugs until something stops that system.
**How to apply:** when a hold/gate is added, list the per-frame systems it skips and check whether each one's first write is
what makes the spawn state valid. For this one, the root fix is spawning with the roll-free rotation (scene_loader ~854 and
entity_spawner ~1498/1565), or writing rotation before the sim gate. Related: [[camera-mode-switch-spawn-only-state]],
[[flycam-dual-authoring-path]].

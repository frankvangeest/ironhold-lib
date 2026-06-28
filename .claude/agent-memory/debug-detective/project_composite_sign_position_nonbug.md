---
name: composite-sign-position-nonbug
description: The "composite child wrong position with opposite-sign XZ" backlog bug — proved transform/physics math is sign-symmetric; the stated symptom cannot arise from transform composition; real defects nearby are collider replacement + nested RigidBody::Fixed
metadata:
  type: project
---

Backlog item "composite prefab child positions wrong with mixed-sign translation" (`loot_display_01` at `(-7,0,4)`).

**Conclusion: the stated symptom (sign-dependent on opposite-sign XZ) cannot be produced by any transform/physics path in this codebase.** All composition is multiplicative Bevy hierarchy (`add_child`) + matrix inverse in bevy_rapier writeback — both are linear and sign-symmetric. Verified with numeric round-trip sims at `(7,0,4)`, `(-7,0,4)`, `(-7,0,-4)`: all reproduce correct world pos and correct local round-trip identically.

**Why:** A bug that triggers specifically on *opposite signs* requires `atan2`/`signum`/`abs`/`x>0`/quadrant logic. Grep of `ironhold_core/src` for those in positioning code found NONE — only change-detection magnitude guards. The backlog's "suspected cause: sign handling / component-wise multiply in scene_loader composite branch" is a misdiagnosis; that branch uses `Transform{translation, rotation: Quat::from_euler, scale}` + `add_child`, no sign ops.

**How to apply:** Before chasing this again, get Frank to re-confirm the actual visual symptom (it may be a GLB pivot/rotation artifact that only *looks* offset at certain parent yaws, or a stale/since-fixed observation). Do not patch scene_loader transform math — it is correct.

**Real defects found in the same path (worth fixing independently of the phantom symptom):**
1. `entity_spawner.rs::spawn_prefab_instance` inserts trigger_zone `Collider::ball`+`Sensor` (line ~88-95) THEN `Collider::compound` for `colliders` (line ~144) on the SAME entity. Second `Collider` insert wins → the trigger-zone sensor ball is silently lost; entity stays a solid compound body that also carries `Sensor`+`COLLISION_EVENTS` (mismatched). chest_01 has both `trigger_zone` and `colliders`, so it is affected. See [[trigger-zone-composite]] for the related composite-path wiring rule.
2. Nested `RigidBody::Fixed`: `loot_display` parent gets `RigidBody::Fixed` from the platform child's `physics:true` (scene_loader ~line 2946), and the chest_01 child ALSO gets its own `RigidBody::Fixed` (entity_spawner ~line 144). A rigid body parented under another rigid body is a bevy_rapier anti-pattern — the child body's pose is driven by writeback (`parent_global.inverse()*world`) every frame and can fight transform propagation. Math round-trips in isolation but it is fragile; prefer attaching the chest's colliders to the existing parent body (no second RigidBody) for composite props.

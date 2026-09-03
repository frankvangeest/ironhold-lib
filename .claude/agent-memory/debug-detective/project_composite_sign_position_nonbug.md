---
name: composite-sign-position-nonbug
description: The "composite child wrong position with opposite-sign XZ" backlog bug — proved transform/physics math is sign-symmetric; the stated symptom cannot arise from transform composition; the remaining real defect nearby is nested RigidBody::Fixed (the trigger_zone/Collider::compound overwrite claim is now stale, see below)
metadata:
  type: project
---

Backlog item "composite prefab child positions wrong with mixed-sign translation" (`loot_display_01` at `(-7,0,4)`).

**Conclusion: the stated symptom (sign-dependent on opposite-sign XZ) cannot be produced by any transform/physics path in this codebase.** All composition is multiplicative Bevy hierarchy (`add_child`) + matrix inverse in bevy_rapier writeback — both are linear and sign-symmetric. Verified with numeric round-trip sims at `(7,0,4)`, `(-7,0,4)`, `(-7,0,-4)`: all reproduce correct world pos and correct local round-trip identically.

**Why:** A bug that triggers specifically on *opposite signs* requires `atan2`/`signum`/`abs`/`x>0`/quadrant logic. Grep of `ironhold_core/src` for those in positioning code found NONE — only change-detection magnitude guards. The backlog's "suspected cause: sign handling / component-wise multiply in scene_loader composite branch" is a misdiagnosis; that branch uses `Transform{translation, rotation: Quat::from_euler, scale}` + `add_child`, no sign ops.

**How to apply:** Before chasing this again, get Frank to re-confirm the actual visual symptom (it may be a GLB pivot/rotation artifact that only *looks* offset at certain parent yaws, or a stale/since-fixed observation). Do not patch scene_loader transform math — it is correct.

**Real defect found in the same path (worth fixing independently of the phantom symptom):**
- Nested `RigidBody::Fixed`: a composite parent gets `RigidBody::Fixed` from a child's
  `physics: true` (`scene_loader.rs::spawn_primitive_children`, ~line 3284), and a nested
  Actor/Prop child with its own `colliders:` (e.g. `chest_01`) gets its own separate
  `RigidBody::Fixed` too (`entity_spawner.rs`, ~line 240,
  `commands.entity(spawned.parent).insert((RigidBody::Fixed, Collider::compound(shapes)))`). A
  rigid body parented under another rigid body is a bevy_rapier anti-pattern — the child body's
  pose is driven by writeback (`parent_global.inverse()*world`) every frame and can fight
  transform propagation. Math round-trips in isolation but it is fragile; prefer attaching the
  chest's colliders to the existing parent body (no second RigidBody) for composite props.

**Claim #1 from the original investigation (trigger_zone's `Collider::ball`+`Sensor` getting
overwritten by a same-entity `Collider::compound` insert) is now STALE — confirmed fixed by
re-reading the current code.** `trigger_zone` is no longer inserted as a component directly on
the prefab's own entity at all: `attach_prefab_features` (`entity_spawner.rs` ~line 91) spawns it
as a **separate sensor child entity** (`commands.spawn((..., Collider::ball(zone_def.radius),
Sensor, ...)).id()` then `commands.entity(entity).add_child(sensor)`), so there is no longer any
entity that receives both a `trigger_zone` sensor `Collider` and a `Collider::compound` — the
overwrite this claim described is structurally impossible now.

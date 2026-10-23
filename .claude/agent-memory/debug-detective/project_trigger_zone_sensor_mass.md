---
name: trigger-zone-sensor-mass
description: Rapier does NOT exclude sensors from body mass; trigger_zone sensor fixed with Density(0.0) on feature/fix_trigger_zone_mass (2026-10-23), but other default-density child colliders under a Dynamic NPC still leak mass
metadata:
  type: project
---

Rapier (rapier3d-0.31 / bevy_rapier3d-0.33) sums every enabled collider attached to a body into its
mass (`recompute_mass_properties_from_colliders` has no sensor check; `.sensor(true)` keeps the
default `Density(1.0)`). bevy_rapier attaches a child collider to the nearest ancestor `RigidBody`.
A 2.5 m sensor ball = 65 kg vs an NPC capsule ~0.45 kg (~146x).

**Status:** the `trigger_zone` sensor in `attach_prefab_features` got
`ColliderMassProperties::Density(0.0)` on `feature/fix_trigger_zone_mass` (2026-10-23). At that
point NO shipped prefab combined `trigger_zone` with a Dynamic body (all Prop/Primitive/bodyless
Actor), so no shipped behaviour depended on the old mass.

**Why it still matters:** the same leak class remains for any other default-density collider that
ends up under a Dynamic NPC: a composite NPC prefab's `physics: true` child (spawn_primitive_children
inserts the child collider + `RigidBody::Fixed` on the parent, then the NPC branch's later
`RigidBody::Dynamic` insert wins, so the "static" child becomes solid moving mass). A single-mesh
primitive with `sensor: true` + `npc` keeps `Sensor` while the NPC capsule replaces the collider,
so the whole NPC becomes a sensor.

**How to apply:** when a diff adds any collider/sensor spawn near a rigid body, ask "which ancestor
body does this attach to, and with what density?". Docs drift: check `docs/20_data_formats.md`'s
`trigger_zone` row too, not only `docs/30`. Related: [[ground-cast-sees-sensors]].

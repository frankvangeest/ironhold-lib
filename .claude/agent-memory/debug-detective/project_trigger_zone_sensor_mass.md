---
name: trigger-zone-sensor-mass
description: A prefab's trigger_zone sensor ball silently adds density-1.0 mass to any ancestor RigidBody::Dynamic (rapier does NOT exclude sensors from mass); harmless on Props/Fixed, catastrophic on NPCs
metadata:
  type: project
---

`trigger_zone: (radius: R)` spawns a child entity with `Collider::ball(R)` + `Sensor` and **no**
`ColliderMassProperties` component (`entity_spawner.rs`, `attach_prefab_features`). Verified through
the dependency sources at rapier3d-0.31.0 / bevy_rapier3d-0.33.0:

- `bevy_rapier` walks the `ChildOf` chain to find the ancestor `RigidBody` (`plugin/systems/collider.rs`
  `collider_offset`), so the sensor becomes a child collider of the *parent's* body.
- With no `ColliderMassProperties` component, the builder keeps rapier's default
  `ColliderMassProps::Density(1.0)` (`geometry/collider_components.rs`). `.sensor(true)` does **not**
  zero it.
- `RigidBodyMassProps::recompute_mass_properties_from_colliders` (`dynamics/rigid_body_components.rs`)
  only checks `co.is_enabled()` and `co.parent` — **there is no sensor exclusion**.

Consequence: a 2.5 m sensor ball = 65.4 kg. An NPC capsule (r=0.3, h=1.8) = 0.45 kg. So
`trigger_zone` on a prefab with an `npc:` component (which gets `RigidBody::Dynamic`) multiplies its
mass ~146x — the NPC becomes effectively immovable and shoves the player instead of being pushed.

**Why:** all shipped `trigger_zone` users before 2026-08 were `kind: Prop` (→ `RigidBody::Fixed`,
mass irrelevant) or a bodyless `kind: Actor` (`merchant_vendor` — no `npc:` block, so no RigidBody,
so the sensor is a static collider). The zombie corpse-loot prefab was the first Dynamic one.

**How to apply:** whenever a diff adds `trigger_zone` to a prefab, check whether that prefab also
has `npc:` (or anything else that inserts `RigidBody::Dynamic`). If so, flag it — and note there is
**no RON knob** to fix it, since the sensor's mass props are hardcoded in `entity_spawner.rs`. The
engine fix is to insert `ColliderMassProperties::Density(0.0)` (or `Mass(0.0)`) alongside the
`Sensor`. Related: [[ground-cast-sees-sensors]] (the other place sensor colliders leak into
non-sensor physics).

---
name: ground-cast-sees-sensors
description: player_movement_system's ground shape-cast uses QueryFilter::new(), which does NOT exclude sensors, so every trigger_zone ball is a toi-0 penetrating hit with a horizontal normal; also, a penetrating hit's normal1 is not unit length
metadata:
  type: project
---

Three measured facts about `player_movement_system`'s downward ground shape-cast
(`capabilities/player.rs`, the `cast_shape` at ~line 299 and the slope gate at ~line 333):

**1. `QueryFilter::new()` includes sensors.** No flags are set by default
(bevy_rapier3d `src/pipeline/query_filter.rs:44` -> `Default`; rapier3d
`src/pipeline/query_pipeline.rs:607` only skips sensors when `EXCLUDE_SENSORS` is set).
`exclude_rigid_body(player)` excludes nothing else. Every `trigger_zone: (radius: r)` prefab
spawns a child with `Collider::ball(r)` + `Sensor`
(`runtime/scene_manager/entity_spawner.rs:90`), so those balls are live ground-cast targets.
`capabilities/npc.rs:144` already calls `.exclude_sensors()` on its line-of-sight ray — the
ground cast is the outlier, not the precedent.

**2. A shape-cast that starts inside another collider returns `time_of_impact == 0`** (parry
`ShapeCastOptions::stop_at_penetration` defaults to `true`), which always beats the real floor's
toi (~0.0089 at shipped defaults: a 0.01 m origin skin). So *any* overlapping collider wins the
cast over the ground the player is standing on, and its EPA normal is whatever direction pushes
the cast ball out — radial/horizontal for a sensor sphere, so ~90 deg from vertical. Measured
veto radius for a `trigger_zone` of radius r: horizontal distance < `r + collider_radius`
(2.9 m for r=2.5 at defaults), independent of the prop's `RigidBody` kind (none/Fixed/Dynamic
all identical). `stop_at_penetration: false` does NOT suppress a fully-contained overlap (no
separating velocity exists), so it fixes solid walls but not sensor spheres.

**3. On a penetrating hit, `normal1` is NOT unit length** (measured |n| = 0.517 for a sensor
sphere). `d.normal1.dot(Vec3::Y).acos()` therefore computes `acos(|n| * cos(theta))`, biased
toward 90 deg — any angle test on a penetrating normal must `.normalize()` first, or it can
report a genuinely flat contact as unwalkable.

**Why:** root-caused 2026-08-23 from a `3rd_person_game_demo` playtest — the falling animation
latching on while standing on flat sand near the loot-display chest. `player_start` is
(0, 1, 10) and `chest_01` (trigger_zone 2.5) is at (-2.5, 0.4, 10.5), 2.55 m away, so the player
spawns *inside* the veto radius. Pre-`uphill_jump_lock` the same sensor hit existed but
`is_grounded` was proximity-only (`hit.is_some()`), so the wrong answer was "always grounded" and
nobody noticed; the walkable-slope gate flipped it into a persistent false negative.

**FIXED** on feature/uphill-jump-lock (`.exclude_sensors()` + `.normalize_or_zero()` on the
ground cast). Verified 2026-08-23 by mutation test: reverting `.exclude_sensors()` fails 5 of the
9 tests in `crates/ironhold_core/tests/prop_ground_veto_tests.rs`, so that file has real
regression value, not just coverage theatre.

**A sensor can never be a legitimate floor, so excluding them costs nothing.** `sensor: true` on a
primitive (`PrimitiveParams.sensor`, `scene_loader.rs:528-531`) inserts `Sensor` *instead of*
`RigidBody::Fixed` ("sensor takes precedence"), so a sensor-only floor has no contact response at
all — measured: the player falls straight through (y = -4.45 after 30 ticks, 0 grounded ticks).
Pre-fix the cast reported "grounded" the whole way down, i.e. the old behavior was a lie, not a
feature. No shipped project authors a sensor floor (the only `sensor: true` in `assets/` is
`primitive_world`'s collectable coin) — and that coin was a *second* instance of this same bug.

**How to apply:** any new physics query in this codebase needs an explicit sensor decision. When
a grounding/collision bug appears "only near props", check the prop's `trigger_zone` radius
first — the prop's *solid* collider only vetoes if the player is actually touching it AND the
collider reaches above the cast ball's centre (feet + `collider_radius` + 0.01); that solid-wall
case is still unfixed and is pinned as an intentional limitation by
`solid_prop_taller_than_cast_ball_centre_still_vetoes_when_pressed_against`.
`player_slope_jump_tests.rs` can never catch this class — every case there spawns exactly one
ground collider and the player, nothing else. `QueryFilter` is a plain `Copy` value built fresh at
each of its only two call sites (`player.rs:328`, `npc.rs:144`) — nothing is cached or shared, and
trigger/collectible detection reads narrow-phase `CollisionEvent`s, a completely separate pipeline
from `cast_shape`, so a filter change here cannot leak into pickup behavior. See
[[slope-jump-test-harness-gotchas]], [[is-grounded-overloaded-three-consumers]],
[[coyote-time-has-a-real-upper-bound]].

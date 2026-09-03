---
name: ground-cast-penetrating-normal
description: The player ground shape-cast's normal1 is an EPA minimum-translation vector, not a surface normal (fixed by lifting the cast origin by collider_radius); plus the second-order consequence — cast_shape returns ONE nearest hit, so once that hit's normal became load-bearing, any collider near the feet can veto the real floor (sensors fixed via exclude_sensors; solid walls still can).
metadata:
  type: project
---

Measured during the third review round of `uphill_jump_lock.md` (2026-08-21), with a throwaway
probe test against real Rapier. **Any future feature that reads a normal, contact point, or
`ShapeCastHitDetails` out of `player_movement_system`'s ground cast must account for this.**

**The cast always starts penetrating.** `spawn_player_entity_core` builds the player collider as
`Collider::compound([(Vec3::Y * (cap_half + cap_radius), .., capsule_y(cap_half, cap_radius))])`
— the capsule bottom sits at the entity origin, so `feet_pos = global_transform.translation()` is
at the contact surface. The ground cast sweeps `Collider::ball(collider_radius)` *centered at
that origin*, so at rest the query ball is buried ~`collider_radius` (0.4 m at defaults) inside
the ground. Status is therefore `PenetratingOrWithinTargetDist` with `time_of_impact == 0.0`
essentially always while standing or walking.

**Consequence: `normal1` is not a surface normal in that state.** With
`compute_impact_geometry_on_penetration: true`, parry takes the penetration branch
(`parry3d/src/query/shape_cast/shape_cast_support_map_support_map.rs:~36`) and returns
`contact_support_map_support_map(.., prediction = Real::MAX)`'s normal — a GJK/EPA **minimum
translation vector** (shortest way to push the shapes apart), not the surface's geometric normal.
Those coincide only when the shortest exit happens to be "straight up", i.e. for a **thick convex
solid** (a `Collider::cuboid` ground plate). They do not coincide for a **zero-thickness
`TriMesh`**, where the shortest exit is sideways toward the nearest triangle edge (edge distance
<= ~0.1 m on terrain_demo's 0.5 m cells, vs 0.4 m for the vertical exit) or straight down.

Measured at a settled capsule pose, `max_walkable_slope_deg = 45`:

| ground | true angle | origin = feet (penetrating) | origin = feet + collider_radius |
|---|---|---|---|
| TriMesh 0.5-cell | 0° | **90.7° → unwalkable** | 0.0° ✓ |
| TriMesh 0.5-cell | 20° | **64.4° → unwalkable** | 20.0° ✓ |
| TriMesh 0.5-cell | 40° | 9.2° | 40.0° ✓ |
| TriMesh 0.5-cell | 60° | **17.0° → walkable** | 60.0° ✓ |
| cuboid (solid) | 0/20/40/60° | 0/20/40/60° ✓ | 0/20/40/60° ✓ |

So on a TriMesh the reported angle has *no monotonic relationship* to the real slope — it's
effectively noise that also flickers with sub-millimetre motion.

**The fix that works** (verified): lift the cast origin to `feet_pos + Vec3::Y * collider_radius`
and set `max_time_of_impact = collider_radius + ground_cast_length`. The ball then starts clear
of the surface, so total reach below the feet is unchanged (`collider_radius +
ground_cast_length`, still 0.7 m at defaults — see [[ground_detection_jump_invariants]]) and the
normal becomes the true face normal. Keep
`compute_impact_geometry_on_penetration: true` even then: at rest the lifted ball still grazes
~1 mm into the surface (`PenetratingOrWithinTargetDist`, toi 0), but with the ball *center* ~0.4 m
above the surface EPA resolves upward correctly. With a lifted origin, a sphere-cast landing on a
triangle *edge* returns a blend of the two adjacent face normals, so walkable/unwalkable
transitions at a seam are smooth — per-triangle flicker is not a concern.

**Where TriMesh ground exists:** `capabilities/terrain.rs:111` is the only
`Collider::from_bevy_mesh(.., ComputedColliderShape::TriMesh(..))` site in the crate, but it
covers all terrain. Scenes with real terrain: `quick_scene/scenes/main.scene.ron` (has a real
`player_warrior` — the flagship gallery project), `terrain_demo/scenes/terrain.scene.ron` (flycam
only, no player), `integration_tests/scenes/terrain_test.scene.ron`.

**Test-coverage trap that hid this:** every case in `tests/player_slope_jump_tests.rs` uses
`Collider::cuboid(60.0, 0.25, 60.0)` — the one geometry family for which the penetrating normal is
accidentally correct. A slope/grounding test that only uses a solid cuboid ground proves nothing
about terrain. Always add a trimesh-ground case (a flat grid trimesh must report grounded; a 60°
grid trimesh must not).

**The bigger structural consequence (found by playtest 2026-08-23, same feature): `cast_shape`
returns exactly ONE hit — the nearest.** Before the walkable-slope gate, `is_grounded =
cast_shape(..).is_some()`, so *which* collider answered never mattered. The gate made that single
hit's normal load-bearing, which silently promoted every collider near the feet into a potential
floor **veto** — a nearer/penetrating hit with a near-horizontal normal permanently masks the real
floor underneath. Two instances:

- **Sensors** (worst case — they penetrate *by design*, so `toi == 0` always beats the floor's
  small-but-nonzero toi). Fixed with `.exclude_sensors()` on the ground `QueryFilter`. This is a
  *schema-invariant* fix, not a workaround: `PrimitiveParams.sensor` is documented as "no physical
  presence" and is mutually exclusive with `physics` (`schema/catalog.rs:~1506`), so a sensor can
  never be legitimate floor. `capabilities/npc.rs`'s LOS raycast already did this — core has only
  two physics queries and now both exclude sensors, so this is the crate-wide rule.
  Don't reach for `CollisionGroups` instead: nothing in the schema or engine uses collision/solver
  groups today, and moving a physics invariant into designer-authored membership bits turns a
  mis-authored group into "player never grounded".
- **Solid geometry — FIXED.** `ground_cast()` (`capabilities/player.rs`) now re-queries in a
  bounded loop (`MAX_GROUND_CAST_CANDIDATES = 4`), excluding via `QueryFilter::predicate` any hit
  that is **both** not underfoot **and** not walkable, until an accepted candidate is found or the
  candidates for that tick are exhausted — this is exactly the "bounded `exclude_collider` re-cast"
  fallback this section proposed, and it landed as the actual fix rather than the cheaper
  witness1-based short-circuit also considered here. "Underfoot" uses the same `witness1`
  world-space contact point this section traces, at or below `feet_pos.y + collider_radius * 0.5`.
  Both conditions (not-underfoot AND not-walkable) are required before exclusion — a first version
  that rejected on "not underfoot" alone was caught in review because it imposed a hidden 60°
  walkable-slope ceiling independent of `max_walkable_slope_deg` (see the "Underfoot-contact test
  geometry" note above for why `k=0.5` ⇒ 60°). Repro test:
  `crates/ironhold_core/tests/prop_ground_veto_tests.rs::solid_prop_taller_than_cast_ball_centre_no_longer_vetoes_when_pressed_against`
  (and its `_on_trimesh_terrain` sibling). A known, distinct, still-open residual gap remains
  (tracked in `planning/backlog.md` ▸ Bugs, not this note's subject): `QueryFilter::predicate`
  excludes by whole `Entity`, so a wall that's part of the *same collider entity* as the walkable
  floor beneath it (a compound-collider prop) still excludes both together.

**`witness1` world-space chain — fully traced 2026-09-01, don't re-derive.** Verified end to end
against the vendored crates for the *filtered* call path `player.rs` actually uses:
`RapierContextSystemParam::cast_shape` (bevy_rapier3d-0.33
`plugin/context/systemparams/rapier_context_systemparam.rs:415`) → `with_query_pipeline` →
`RapierQueryPipeline::new_scoped` (`plugin/context/mod.rs:245`) →
`rapier3d::QueryPipeline::cast_shape` (`pipeline/query_pipeline.rs:482`) →
`CompositeShapeRef::cast_shape` (parry3d-0.25.3
`query/shape_cast/shape_cast_composite_shape_shape.rs:14`), whose leaf callback gets
`part_pose1 = co.position()` — the collider's **world** isometry (rapier3d
`query_pipeline.rs:122-134`, `map_untyped_part_at`) — and returns `hit.transform1_by(part_pose1)`,
which maps `witness1`/`normal1` (only those two; `witness2`/`normal2` are left alone) into world
space (parry3d `query/shape_cast/shape_cast.rs:83-92`). **No further transform is applied**, so
`witness1` is genuinely world-space in `player.rs`. Nested composites (compound colliders, TriMesh
sub-parts) compose correctly — the inner hit is in the part frame and the outer `transform1_by`
lifts it to world.
**Citation correction to the older note above:** the authoritative "witness and normal 1 refer to
the world collider, and are in world space" doc is at **bevy_rapier3d `src/plugin/context/mod.rs`
lines 475-478** (on `RapierQueryPipeline::cast_shape`). Two nearby occurrences are *not* usable
citations: `mod.rs:520` is inside a commented-out `/* TODO */` block for the disabled
`nonlinear_cast_shape`, and rapier3d `query_pipeline.rs:496` documents `cast_shape_nonlinear`.

**`QueryFilter::predicate` ANDs, never overrides** (rapier3d-0.31 `query_pipeline.rs:681-691`):
`test()` is `exclude_collider && exclude_rigid_body && groups && flags.test() && predicate`. So
`.exclude_rigid_body(..).exclude_sensors().predicate(..)` all apply together. bevy_rapier wraps the
`Fn(Entity)` predicate via `to_rapier_query_filter_predicate` (`plugin/context/mod.rs:234`), which
resolves the entity with `RapierContextColliders::entity_from_collider` — **the exact same mapping
`cast_shape` uses for its returned `Entity`**. That identity is what makes an
"exclude-the-last-hit-and-re-cast" loop guaranteed to terminate. Filtering happens in
`map_untyped_part_at` *before* the narrow phase, so an excluded leaf is genuinely skipped and the
next-nearest hit surfaces.

**Underfoot-contact test geometry (derived 2026-09-01, `feature/prop-ground-veto`).** With the
lifted cast (ball radius `R = collider_radius`, origin `feet + (R + 0.01)·Y`): a *flat* floor gives
`witness1.y == feet.y`; a slope of angle θ gives `witness1.y == feet.y + R·(1 − cos θ)`; a *vertical
wall* the player is pressed against gives `witness1.y ≈ feet.y + R` (EPA's MTV is horizontal, so
point1 sits at the ball centre's height). A tolerance of `R · k` therefore imposes a hard walkable
slope ceiling of `acos(1 − k)`, **independent of `collider_radius`** (both sides scale with R):
`k = 0.5` ⇒ exactly 60°. Above that the real floor's own contact reads as "not underfoot".

**`QueryFilter` cost:** `QueryFilterFlags::test` (rapier3d-0.31 `pipeline/query_pipeline.rs:601`)
is a bitflag + `collider.is_sensor()` bool per *candidate collider*, evaluated before narrow phase
— so `.exclude_sensors()` is strictly cost-*negative* here: it removes sensor leaves from the
`compute_impact_geometry_on_penetration` GJK+EPA path (which allocates per call, see below).

**API facts worth not re-deriving** (bevy_rapier3d 0.33 / rapier3d 0.31 / parry3d 0.25.3):
- `ShapeCastHitDetails::normal1` *is* the hit collider's outward normal *in world space* for
  `RapierContext::cast_shape`. The struct's own doc comment (`bevy_rapier3d/src/geometry/mod.rs:101`,
  "local-space outward normal on the first shape") is copied verbatim from parry and is
  misleading here; the method doc (`src/plugin/context/mod.rs:478`) is authoritative. Mechanism:
  rapier passes its `QueryPipeline` as parry's *composite shape 1*
  (`rapier3d/src/pipeline/query_pipeline.rs:~489`), and each leaf hit is mapped back with
  `hit.transform1_by(part_pose1)`
  (`parry3d/src/query/shape_cast/shape_cast_composite_shape_shape.rs:53`) into the composite's
  frame = world space.
- `compute_impact_geometry_on_penetration: true` is **not purely additive**: the penetration
  branch ends in `contact_support_map_support_map(..)?` on an `Option`, so an EPA failure
  (`GJKResult::NoIntersection`) turns what would have been `Some(hit)` into `None` — it can flip a
  hit into a miss, not just populate `details`. It also runs a second full GJK+EPA query (with a
  per-call `EPA::new()` allocation) per penetrating candidate leaf — on a TriMesh that is per
  candidate triangle, not once per cast.
- `ShapeCastStatus` semantics are unaffected by the flag; `Failed` maps `details` to `None`
  regardless (`bevy_rapier3d/src/geometry/mod.rs:113`).

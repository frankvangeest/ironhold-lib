---
name: rapier-query-and-sync-facts
description: Verified parry3d 0.25.3 / bevy_rapier3d 0.33 facts for any new shape-cast or Transform-write on the player body — stop_at_penetration default, Writeback writes Transform only (GT stale on 2nd tick), move_shape is a pure query, raw_grounded is proximity not contact
metadata:
  type: project
---

Verified 2026-10-02 during plan-review of `step_offset_auto_step.md` (source-read, not run).

- **`ShapeCastOptions::default().stop_at_penetration == true`** (`parry3d-0.25.3/src/query/shape_cast/shape_cast.rs:141`).
  Any cast whose start pose already overlaps a collider returns toi 0 *regardless of direction*
  (`shape_cast_support_map_support_map.rs:36-53`). With `false`, a toi-0 contact whose `normal1·vel >= 0`
  (separating) is discarded. Any multi-cast probe from a pose resting on floor / pressed into a wall must set this
  per cast (up/away casts: false; "what am I pressed against" casts: true).
- **bevy_rapier `writeback_rigid_bodies` writes `Transform` only, not `GlobalTransform`** (`plugin/systems/rigid_body.rs:403+`).
  GT is re-propagated by `RapierTransformPropagateSet` inside the *next* tick's `SyncBackend` (after the gameplay chain).
  So on the 2nd tick of a multi-tick frame, a root body's GT is one step stale; `Transform` is the authoritative pose.
  Probe from `Transform` if you then write `Transform` (mixing gives off-by-one-tick Y → penetration pops).
  `mark_dirty_trees` only flags; it does not propagate.
- Transform write on a Dynamic body → `apply_rigid_body_user_changes` → `rb.set_position(.., wake)` when GT differs
  from `last_body_transform_set`. Same path every player turn tick already uses (no new glam-backend exposure).
- **`RapierContextSimulation::move_shape` (`plugin/context/mod.rs:874`) runs the KCC incl. autostep as a pure query on
  any shape** — "Rapier autostep needs a kinematic body" is false; it needs `&mut` simulation ctx + `RapierQueryPipelineMut`.
- `raw_grounded` is a 0.7 m proximity sensor (see [[airborne_ground_reacquisition]]); any feature that needs "standing
  on the ground" must add a contact-gap check on the retained ground hit, not reuse `raw_grounded && jumps_used == 0`.

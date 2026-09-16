---
name: project-deterministic-fixed-timestep
description: Rapier moved from PostUpdate/TimestepMode::Variable into FixedUpdate/Fixed{1/64} — what that changes for WASM frame cost (SyncBackend carries a FULL Bevy transform propagation pass, now per-tick), catch-up burst amplitude, and 64Hz render stepping with no interpolation
metadata:
  type: project
---

Branch `feature/deterministic_fixed_timestep` (v1 plan `planning/features/deterministic_fixed_timestep.md`),
reviewed 2026-09-15 while still uncommitted. Verify against current `capabilities/physics.rs` before
relying on any of this — it may have been revised or dropped.

**The change:** `RapierPhysicsPlugin::default().in_fixed_schedule()` + a pre-inserted
`TimestepMode::Fixed { dt: 1.0/64.0, substeps: 1 }`; the existing `FixedUpdate` gameplay chain and
`motion_system` (moved out of `Update`) ordered `.before(PhysicsSet::SyncBackend)`.

**The non-obvious cost fact — `PhysicsSet::SyncBackend` contains a full Bevy transform propagation
pass.** Verified in `bevy_rapier3d-0.33.0/src/plugin/plugin.rs:138-145`: `get_systems(SyncBackend)`
includes `bevy::transform::systems::{sync_simple_transforms, propagate_parent_transforms}` inside
`RapierTransformPropagateSet`. These are whole-world systems, not physics-entity-scoped — every
`Transform` root and every hierarchy (GLB skeletons, UI trees, label anchors) is walked. In
`PostUpdate` that was exactly one extra propagation per rendered frame; in `FixedUpdate` it is one
per *tick*. So the extra-propagation count per frame is now `ticks_this_frame`, not 1:
~0.44/frame at 144 Hz (a win), ~1.07 at 60 Hz (a wash), ~2 at 32 Hz, and up to the catch-up cap on
a hitch. On single-threaded WASM `propagate_parent_transforms` has no task-pool parallelism to hide
behind.

**Rapier step count is now decoupled from frame rate.** `TimestepMode::Fixed` does exactly one
`pipeline.step` per tick of its schedule (`plugin/context/mod.rs:809`), no internal catch-up loop —
the loop is Bevy's `FixedMain`. Old `Variable{max_dt: 1/60}` stepped exactly once per rendered
frame and degraded into slow-motion below 60 fps instead of costing more. Net: cheaper above 64 fps,
more expensive below it — i.e. more expensive precisely where WASM already struggles.

**Catch-up burst amplitude grew.** `Time<Virtual>::max_delta` is still Bevy's 250 ms default
(nothing in `ironhold_core` overrides it; `capabilities/camera.rs:19` only documents it), so one
frame can run ~16 fixed ticks. That burst already carried the gameplay chain (see
[[project-ground-cast-loop]]); it now also carries 16x {2 transform propagations + collider/body
user-change scans + solver step + writeback}. Lowering `max_delta` to ~100 ms is the cheap guard.

**No render interpolation.** bevy_rapier's `TransformInterpolation` component is only honored by
`TimestepMode::Interpolated`, not `Fixed`. Cameras run in `Update` (`lib.rs` ~line 320) which sits
after `RunFixedMainLoop`, so on a >64 Hz display consecutive frames render identical dynamic-body
poses — 64 Hz stepping on the player mesh, props, and `motion_system` spinners.
`follow_camera_system`'s exponential smoothing masks it for that one mode; `camera_orbit_system`
does not smooth position.

Supersedes the `TimestepMode::Variable` / PostUpdate claims in
[[project-player-friction-toggle]] and [[project-player-spawn-unification]] if this lands.
See [[project-libm-det-math]] for the math half of the same change.

---
name: update-side-globaltransform-staleness
description: The Update-scheduled GlobalTransform staleness class introduced by fixed-timestep physics, the fresh_global_transform workaround, and which sibling call sites are still unfixed
metadata:
  type: project
---

Reading `&GlobalTransform` from an **`Update`**-scheduled system is now a recognised bug class, not a neutral choice. `GlobalTransform` is only written by Bevy's `PostUpdate` propagation and by Rapier's `SyncBackend` (start of a `FixedUpdate` tick). An `Update` system therefore sees the *previous* frame's pose, while `Transform` on the same entity has already been updated this frame (camera mode systems) or by the last physics tick (players/NPCs). On a frame that runs two `FixedUpdate` ticks the two diverge visibly.

**Why:** landed during `planning/features/deterministic_fixed_timestep.md` v1 (physics moved to `FixedUpdate` @ 64Hz). Double-tick frames are **routine, not hitch-only** — `Window`'s default `PresentMode::Fifo` means the real frame rate is the display's refresh, and no common refresh rate evenly divides 64Hz (~4 double-tick frames/s at 60Hz, ~16/s at 144Hz). The native `FramepaceSettings` cap being set to `FIXED_TICK_RATE` does *not* eliminate this; the `start_app` code comment says so explicitly, but `crates/ironhold_core/src/CLAUDE.md`'s fixed-timestep section and the feature plan's own task bullet both still claim the cap makes a normal frame advance exactly one tick. **Treat that claim as stale until those two are corrected** — it is exactly the reasoning someone would use to wrongly conclude a new `Update`-side `GlobalTransform` read is safe.

The workaround is `crate::utils::fresh_global_transform(Option<&Transform>, &GlobalTransform, Option<&ChildOf>)`: for a root entity it returns `GlobalTransform::from(*transform)` (bit-identical to what `sync_simple_transforms` would produce), otherwise falls back to the stale component. `&Transform` must stay `Option` at call sites — some test fixtures carry `GlobalTransform` without `Transform`, and a hard requirement drops them from the query entirely.

**How to apply:** when reviewing any new or changed `Update`-scheduled system that reads `&GlobalTransform` of a player/camera/NPC/prefab, flag it and point at this helper. Known sibling call sites that were **not** converted and still read stale poses: `capabilities/camera.rs`'s `fixed_camera_system` (`look_at_entity` target), `capabilities/particle_renderer.rs`'s billboard camera basis, `capabilities/targeting.rs`'s click-ray camera + selectable positions. `FixedUpdate` readers (`npc.rs`, `player.rs`) are covered instead by the `mark_dirty_trees` system folded into the `FixedUpdate` chain — do **not** use the helper there.

Related: [[schedule_update_vs_fixedupdate]], [[determinism_networking]], [[camera_pose_writer_taxonomy]], [[world_space_widgets]].

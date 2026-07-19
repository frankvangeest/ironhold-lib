---
name: target-indicator-double-despawn
description: target_indicator_system iterates the same `existing` query snapshot in two loops with deferred despawns and no dedup, so one ring can be despawned twice in a single run
metadata:
  type: project
---

`target_indicator_system` (crates/ironhold_core/src/capabilities/target_indicator.rs) has a latent double-despawn. It queries `existing: Query<(Entity, &TrackingTarget)>` once, then iterates that ONE snapshot in two separate loops:
- Loop 1 (~line 119): despawns a ring whose tracked target's `GlobalTransform` is gone (target entity despawned).
- Loop 2 (~line 133): despawns a ring whose `owner` is a player with `Changed<PlayerTarget>` this frame.

Commands are deferred, so the snapshot still contains the ring in loop 2 after loop 1 queued its despawn. If both conditions hit the same ring in one frame (ring's target entity despawned AND owning player's PlayerTarget changed same frame — e.g. retarget in the exact frame the old target dies), the ring is queued for despawn twice → at flush: "Entity NNNv0 is invalid; its index now has generation 1." Intermittent/timing-dependent — shows up occasionally in local_coop combat playtests.

**Why:** classic Bevy hazard — multiple passes over a single query snapshot with `EntityCommands::despawn` (deferred) and no dedup set / `try_despawn`.
**How to apply:** When a symptom is a double-despawn "generation 1" warning, first suspect systems that iterate one query snapshot more than once and despawn in each pass. Bevy 0.18 offers `try_despawn()` (not yet used in this codebase) as the silent-safe variant; the robust fix is to collect ring entities to despawn into a `HashSet<Entity>` and despawn each once. `target_auto_clear_system` only clears on `Visibility::Hidden` (alive entity), NOT on despawn — so death-by-despawn does not auto-clear PlayerTarget, which is why the coincidence requires an external retarget. Related risk areas: [[project_ui_pick_blocking]].

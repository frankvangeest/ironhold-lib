---
name: scene-load-resource-threading
description: Why deferred Commands::insert_resource at scene load is safe to read from later Update systems — the sync-point + same-command-queue atomicity argument, and the one unordered spawn path that breaks it
metadata:
  type: project
---

Scene-load resources (`ActiveSplitScreen`, `ActiveSplitSlotCount`, `DynamicSplitConfig`,
`TargetRingVisibilityMode`, `LoadedTargetIndicator`, ...) are written by **deferred**
`Commands::insert_resource` inside `spawn_players_and_camera`/`scene_loader`. Reviewers repeatedly
ask "can a later system read a stale value?" — the answer is no, for two reasons that are stronger
than the "it runs in an earlier frame" rationale that keeps showing up in code comments:

1. **Sync point on the explicit ordering edge.** `spawn_scene_v2` is registered
   `.before(message_interpreter_system)` (lib.rs), and Bevy's `auto_insert_apply_deferred`
   (default on) inserts an `ApplyDeferred` on that dependency edge. The whole
   `message_interpreter_system → ... → action_executor_system → drain_spawn_queue_system` chain
   therefore sees scene-load resource writes **in the same frame**, not one frame later.
2. **Same-command-queue atomicity.** The player entities and the scene-load resources are queued
   from the *same* system's command buffer, so they are applied together. No observer can ever see
   the new players without also seeing the new resources. This is the load-bearing guarantee — it
   survives system reordering, whereas "runs in a later frame" does not.

**The one exception to watch:** `spawn_player_when_terrain_ready` (entity_spawner.rs) also calls
`spawn_players_and_camera`, but it sits in lib.rs's *unordered* tuple — only `spawn_scene_v2`
carries the `.before(message_interpreter_system)` constraint. Argument (2) still saves it, but
argument (1) does not. If terrain scenes ever gain multi-player split-screen, add the same
`.before(message_interpreter_system)` for symmetry.

**Why to apply:** when reviewing a new scene-load resource, don't demand it be threaded as a
function parameter instead of a resource read. Do flag any code comment that justifies a resource
read with frame separation — reword it to the atomicity argument, since the frame-separation claim
invites someone to "fix" the wrong thing later.

Related: [[split-screen-and-shared-mouse]], [[render-layers-reserved-scheme]].

---
name: deferred-despawn-double-queue
description: Anti-pattern class — one query snapshot iterated twice within a single system run + deferred Commands.despawn() = same entity despawned twice; Bevy 0.18 semantics
metadata:
  type: project
---

Recurring latent-bug class: a system reads ONE `Query` snapshot and iterates it more than once
within a single system run, calling `commands.entity(e).despawn()` in each pass. Because Bevy
`Commands` are deferred (not applied until the schedule flush), an entity queued for despawn in
pass 1 is still present in the same snapshot for pass 2, so the same entity gets queued for
despawn twice.

**Why:** confirmed real in `target_indicator_system` (dead-target cleanup pass + owner-retarget
pass share one `existing: Query<(Entity,&TrackingTarget)>`), fixed on `fix/target-indicator-double-despawn`
with a per-run `HashSet<Entity>` guard. During that review, the SAME shape was found un-fixed in
`runtime/scene_manager/action_executor.rs` (the executor drains the whole ActionQueue in one system
run, so multiple actions in one frame hit the same snapshot): `StopMusic`+`PlayMusicLoop` (or two
music actions) both despawn the same `bg_music_query` entities; two `Action::Despawn(same_id)` both
match via `find` over the `spawned` query snapshot (registry removal doesn't guard the find);
`UnloadOverlay`+`ToggleOverlay(active)` both iterate `overlay_entities`.

**Bevy 0.18 semantics (load-bearing):** `EntityCommands::despawn()` uses the `warn` error handler
explicitly (`bevy_ecs .../system/commands/mod.rs:1864`), NOT the default `panic` handler — so a
double-despawn only logs `"...does not exist"` at WARN, never panics, in both game and test apps.
`try_despawn()` (same file ~1878) does the identical thing but SILENCES the warning — it is the
idiomatic one-call-site fix for "this entity may already be gone" and would remedy this whole class
(target_indicator AND action_executor) more simply than a HashSet.

**How to apply:** when reviewing any system that despawns from a query iterated more than once per
run (or a queue-draining executor), flag it. Because it's benign (warn, not panic), triage as
log-to-backlog, not a blocker — but a "regression test" for it that only asserts end-state + no-panic
will PASS on the buggy code (the warning is the only observable difference); a real regression test
must capture WARN-level tracing events. Determinism is unaffected: these guards use `HashSet::insert`
bool returns only, never iterate the set. See also [[fragile_modules]].

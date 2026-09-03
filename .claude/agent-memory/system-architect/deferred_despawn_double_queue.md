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

**Second, distinct sub-class — recursive despawn over a flat tag sweep (fixed `fix/level-entity-recursive-despawn`, 2026-07-20):** `scene_loader.rs::spawn_scene_v2`'s `level_entities: Query<Entity, With<LevelEntity>>` teardown sweep double-despawned because Bevy 0.18 `despawn()` is RECURSIVE and several widgets tag their cosmetic CHILDREN as separate `LevelEntity` entities attached via `add_child` — confirmed in `capabilities/nameplate.rs` (anchor + shadow + name + per-stat bar bg/fill, all `LevelEntity`) and `capabilities/stat_display.rs` Pixel bars (anchor + border + bg + fill, all `LevelEntity`). Flat query visits parent AND child as separate rows; despawning the parent recursively removes the child, then the loop's later row for that child double-despawns. Fixed with `try_despawn()` (right call — a `Without<ChildOf>` root-only filter is MORE fragile: it assumes every LevelEntity child's ancestor chain ends in a swept LevelEntity root, an unenforced invariant that leaks if violated). This is a DESIGN SMELL but a legitimate resting state: child tagging is redundant (recursion alone would clean them) but deliberate defense against orphaning; that redundancy is exactly what causes the double-visit, so the sweep MUST use try_despawn. Contrast `dialogue.rs::despawn_choice_buttons` — only the root marker is queried, children untagged, cleaned by recursion (the "de-tagged" alternative). All five `action_executor.rs` sites also converted here.

**Overlay sweep caveat (corrects the in-code comments):** `OverlayEntity` is tagged ONLY on roots (backdrop + UI-Root, `scene_loader.rs:~1067/1086/1153`) — overlay descendants spawned via `with_children` are NOT `OverlayEntity`-tagged. So the overlay sweep has NO recursive-children hazard; the fix's comments claiming "overlay trees attach OverlayEntity-tagged descendants" are FACTUALLY WRONG. `try_despawn()` there is still justified, but by the FIRST sub-class (shared-snapshot reuse: `UnloadOverlay`+`ToggleOverlay` iterate one captured `overlay_entities` snapshot), not by recursion. `nameplate.rs`'s cleanup system (`RemovedComponents<NameplateTag>`-driven anchor teardown) now uses `try_despawn()` too — it was the last un-converted sibling of this class and has since been fixed. As of this update, no known site in this class still uses bare `.despawn()` where a double-despawn is plausible; re-grep `\.despawn()` (excluding `try_despawn()`) across `capabilities/` and `runtime/scene_manager/` before assuming this is exhaustive.

**How to apply:** when reviewing any system that despawns from a query iterated more than once per
run (or a queue-draining executor), OR any flat `With<Tag>` sweep where that tag is also applied to
`add_child` children (recursive-despawn sub-class), flag it. Because it's benign (warn, not panic),
triage as log-to-backlog, not a blocker — but a "regression test" for it that only asserts end-state
+ no-panic will PASS on the buggy code (the warning is the only observable difference); a real
regression test must capture WARN-level tracing events, and this suite has NO infra for that (Bevy's
internal error handler logs via the `log` crate facade, not `tracing`; `setup_test_app` adds no
`LogPlugin` bridge). Determinism is unaffected. See also [[fragile_modules]].

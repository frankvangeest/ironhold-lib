---
name: loadscene-teardown-atomicity
description: Action::LoadScene despawns nothing itself; teardown is one atomic fresh-query flush in spawn_scene_v2 — this makes several suspected double-despawn races non-bugs
metadata:
  type: project
---

`Action::LoadScene` (action_executor.rs) despawns **no entities**. It only sets `NextState::LoadingScene`, inserts `SceneHandleV2`, and clears some resources. The actual world teardown is a single `for entity in level_entities.iter() { despawn() }` loop in `spawn_scene_v2` (scene_loader.rs ~169), which reads a **fresh** query each run and fires **exactly once** per load (gated by state==LoadingScene + asset-ready, then `next_state→InGame` blocks re-entry).

**Why this matters for double-despawn investigations:**
- Two `Action::LoadScene` in the same frame is genuinely a harmless no-op re-trigger: both re-insert the *same* asset-server handle (deduped by path) and both set the same state → still ONE teardown. The portal-double-trigger comment ("second LoadScene is a harmless no-op") is **accurate at the Bevy-command level**.
- Because the teardown despawns a tracked entity (e.g. a player with `NameplateTag`) AND its dependent entity (e.g. the `LevelEntity` nameplate anchor, or the target-indicator ring) in the **same command flush**, a `RemovedComponents`-driven cleanup (`nameplate_cleanup_system`) or a per-frame despawner (`target_indicator_system`) can never observe "tracked entity gone but dependent still alive" across a scene load. So the cross-system teardown-vs-cleanup double-despawn race that looks plausible on paper is self-protected — both entities die atomically. `nameplate_cleanup_system` only actually fires when a nameplated entity is despawned **mid-scene** by some *other* mechanism (Action::Despawn), and even then it's the sole despawner of the anchor.

**How to apply:** When a generic Bevy "Entity ... is invalid; generation N" despawn-handler warning appears during a scene transition, do NOT assume the transition caused it. Rule out the current scene's despawn topology first (does it even contain a two-mechanism-despawn entity — target indicator, nameplate anchor, music, overlay?). A bare scene (only players/ground/portal/labels) has only the single-path `LevelEntity` teardown and cannot produce this warning. The warning likely originated in an earlier-traversed scene whose machinery (target indicator / nameplate) is the real source — see [[target-indicator-double-despawn]] and [[deferred-despawn-double-queue]] for two concrete instances of this despawn-topology class.

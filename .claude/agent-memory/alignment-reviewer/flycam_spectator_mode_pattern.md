---
name: flycam-spectator-mode-pattern
description: Flycam+player "spectator mode" (SuppressPlayerCameras/CameraSpawnMode) — the entity-presence-as-sole-switch pattern, and why suppression has no runtime RON un-do
metadata:
  type: project
---

Reviewed 2026-08-17 (verdict ALIGNED). Feature: `planning/features/flycam_scene_conflicts.md`.

**The pattern (a good one to reuse): derived-state resource, not a new RON field.** A scene
combining `tags: ["player"]` + `tags: ["flycam"]` now makes the flycam the sole camera.
`SuppressPlayerCameras(pub bool)` (scene_manager/mod.rs ~210) + `pub(crate) enum CameraSpawnMode
{Spawn, Suppressed}` are pure *derived* runtime state — computed only from `flycam_start.is_some()`
in scene_loader.rs (~772), never authorable. Correct call: the designer knob is the scene's
`entities:` list itself, so zero new schema surface and zero recompile. **Precedent worth citing
when a plan proposes a new RON bool for something already inferable from scene content.**

Insert/reset discipline that made it safe (copy this for any new scene-scoped resource):
`Action::LoadScene` resets it (action_executor.rs:62, beside the 4 other camera resets);
scene_loader inserts the real value **unconditionally** (not only when true) inside the
`if !is_overlay` block (ends scene_loader.rs:1122), so overlays can't clobber it and no scene
inherits stale suppression. `init_resource` in lib.rs ~165 covers app startup.

**Structural gotcha this feature exposed:** `spawn_players_and_camera`'s `Suppressed` early-`return`
sits after the player loop but **before** the split/party diagnostic warns (entity_spawner.rs
~672-714: split+party mutual exclusion, Grid+dynamic, `own_viewport_only` layer collisions). Those
warns silently stop firing in any flycam scene. Partially compensated by a new scene_loader warn
that only inspects `player_configs.first()`'s `split`/`party` presence. Any future diagnostic added
below that early return inherits the same blind spot.

**Known one-way door (documented, not fixed):** in spectator mode the player camera is never
spawned, so there is **no RON action that hands camera control back to the player mid-scene** —
`Action::SetCameraMode` can only re-mode the *existing* flycam entity, which carries
`CameraTargets::default()` (empty), so Follow/Orbit lands targetless. The only recovery is
`Action::LoadScene` into a flycam-free scene. Also `Action::Spawn`/`JoinPlayer` of a player prefab
ignores `SuppressPlayerCameras` entirely and spawns a competing full-window camera.

Note the flycam spawn site NOW inserts `AuthoredCameraMode` (scene_loader.rs ~842) — the
[[camera_modes_v2_pattern]] blocker about `SetCameraMode` being a silent no-op in flycam scenes is
fixed as of this change. Update that memory's claim if re-reading it.

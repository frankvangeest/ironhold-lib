---
name: scenehandle-overlay-never-restored
description: LoadSceneOverlay/ToggleOverlay overwrite SceneHandleV2 with the overlay scene and nothing ever restores it, so anything reading scene_handle (Action::JoinPlayer) permanently reads the overlay, while LoadedSpawnPoints/ActiveSplitSlotCount still hold the base scene's
metadata:
  type: project
---

`Action::LoadScene`, `LoadSceneOverlay` **and** `ToggleOverlay`'s open branch all
`commands.insert_resource(SceneHandleV2(handle))` (`action_executor.rs:67/76/96`). `UnloadOverlay`
and `ToggleOverlay`'s close branch only `try_despawn()` the overlay roots — **neither restores
`SceneHandleV2`**. So once any overlay has been opened in a scene, `scene_handle` points at the
overlay scene for the rest of that scene's life.

Meanwhile `scene_loader.rs:123` inserts `LoadedSpawnPoints`/`LoadedCameraModes` only in the
**Replace** branch (the `if is_overlay` branch deliberately doesn't, so an overlay can't clobber the
live world), and `ActiveSplitSlotCount` is reset only by `Action::LoadScene`.

**Why:** this splits "the current scene" across two different scenes for any system that reads both.
Concretely `Action::JoinPlayer` reads `join_prefab_keys` from `scenes.get(&scene_handle.0)`
(`action_executor.rs:1692`) but `player_{N}_start` from `spawn_points.0` (`:1754`) — after an overlay
it reads join keys from the overlay scene (almost always empty → "scene has no join_prefab_keys
entry for slot N") and spawn points from the base scene. No shipped project triggers it today
(`local_coop_demo` authors no overlay), which is why it has never been seen.

**How to apply:** do not accept "both fields live on one `GameSceneV2`, so no cross-scene
reachability approximation is needed" for anything that reads `scene_handle` — that premise is false
whenever an overlay is involved. When reviewing a new `scene_handle` read, ask what happens after a
pause/inventory overlay has been opened once. Related: [[loadscene-teardown-atomicity]].

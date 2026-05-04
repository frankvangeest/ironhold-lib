# Feature: Scene Preloading

_Status: Ready_
_Planned at: `91cd464` (2026-04-27)_

## What
`Action::PreloadScene(path)` already exists and issues an asset handle in the background.
What's missing is the completion signal and the full fast-path when `LoadScene` fires
on an already-ready handle. This feature closes that loop:

- A polling system watches `PreloadedScenes` handles and emits `scene.preloaded:{name}`
  when all assets for that scene are fully loaded.
- `Action::LoadScene` checks `PreloadedScenes` first; if the handle is ready it skips
  re-issuing the load and transitions immediately (no `LoadingScene` state pause).
- `Action::PreloadScene` also accepts a `warm_assets: true` hint that issues load calls for
  every GLB and texture referenced in the scene's prefab catalog and assets.ron, not
  just the scene RON itself.

## Why
`Action::PreloadScene` was added speculatively but never completed. Without the completion
signal there is no way to know when preloading is done, so authors cannot trigger a
seamless scene switch. Completing this unblocks:
- World-streaming: preload the next zone while the player is in the current one.
- Cutscene flow: preload the post-cutscene scene before the cutscene ends.
- Works synergistically with the loading screen: if preloading finishes early, the
  loading screen shows for zero time.

## Approach

### Completion polling system
New system `preload_poll_system` runs in `Update` (all states). For each handle in
`PreloadedScenes`, calls `asset_server.is_loaded_with_dependencies(handle)`.
When true, emits `SceneEvent::Preloaded(path)` which the message interpreter maps to
`scene.preloaded:{name}`. Removes the handle from `PreloadedScenes` once emitted.

### LoadScene fast-path
In `action_executor_system`, when `Action::LoadScene(path)` fires, resolve the path
and check if `PreloadedScenes` already contains a loaded handle for it. If yes, set
`SceneHandleV2` directly and skip issuing a new `asset_server.load()`. The
`spawn_scene_v2` system then sees the handle as already loaded and spawns immediately
without waiting for `AssetEvent::LoadedWithDependencies`.

### Asset warming (`warm_assets`)
Extend `Action::PreloadScene` to accept `Action::PreloadSceneWarm(path)` (or a flag). The
executor loads the scene RON handle, waits one frame for it to parse, then issues
`asset_server.load()` for every model path in the scene's asset catalog and prefab
catalog. These handles are not stored in `PreloadedScenes` (they go into the asset
server cache automatically). This covers the GLB decode cost.

Doing this in one executor tick is not possible (scene RON not yet parsed). Options:
- A) Store a `PendingWarmScene(Handle<GameSceneV2>)` resource; a second system picks
  it up next frame once the RON is loaded and issues the sub-loads.
- B) Skip v1; just preload the scene RON. GLB decode still happens at spawn time but
  is faster if the bytes are already in the OS file cache.

Recommendation: ship v1 with RON-only preloading. Add asset warming as a follow-up
(it needs the `PendingWarmScene` resource and a new system stage).

## Tasks
- [ ] `preload_poll_system`: poll `PreloadedScenes` handles, emit `scene.preloaded:{name}`, remove when loaded
- [ ] Wire `SceneEvent::Preloaded` → `scene.preloaded:{name}` in the message interpreter
- [ ] `LoadScene` fast-path: check `PreloadedScenes` before issuing a new load
- [ ] Tests: preload a scene, verify `scene.preloaded:*` fires, verify `LoadScene` skips the load state
- [ ] Docs: update `30_runtime_events_and_logic.md` with `scene.preloaded:{name}`

## Open questions
- Should `preload_poll_system` run in all `AppState`s or only `InGame`? Probably all states, so preloading can start from the project loader.
- Asset warming (v2): worth a separate feature file or just a follow-up task here?

## Acceptance criteria
- Given `Action::PreloadScene("scenes/zone2.scene.ron")` fires while in zone 1, the event `scene.preloaded:zone2` fires when zone 2's assets are fully cached.
- Given `scene.preloaded:zone2` has fired, when `Action::LoadScene("scenes/zone2.scene.ron")` fires, the scene transitions without entering `LoadingScene` state.
- Given no prior preload, `LoadScene` behaves exactly as before (no regression).

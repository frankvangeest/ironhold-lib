# Feature: Loading Screen

_Status: Ready_
_Planned at: `7dee9ac` (2026-04-27)_

## What
While a scene or its assets are loading, the engine displays a loading overlay
instead of a frozen black window. The overlay shows at minimum a "Loading…" label;
projects can replace it with a custom scene (splash art, tips, animated logo) by
setting `loading_scene` in their project config. When loading is complete the overlay
is removed and the game scene takes over.

## Why
Currently the window freezes with no feedback during terrain mesh generation or GLB
loading. This is the single biggest barrier to a shippable demo. Even a plain text
overlay unblocks the UX for Beta 0.4 demos.

There is also a web-specific performance reason: Bevy compiles WebGPU render pipelines
synchronously on the main thread on WASM (no async path available). A scene with many
unique custom shaders (e.g. custom_materials has 17 unique shaders) can stall the main
thread for 1–2 seconds during the spawn frame. This registers as a 1400+ ms INP in
browser performance tools — a Core Web Vitals failure. The loading screen fixes this by
keeping the canvas non-interactive while compilation runs, so no user interaction lands
on the compilation frame.

## Approach

### Engine-level overlay
The scene manager already has `AppState::LoadingScene` and `AppState::LoadingProject`.
A new system `loading_screen_system` runs in those states and spawns a full-screen
overlay entity (tagged `LoadingOverlay`, not `LevelEntity`) if one doesn't exist.
It despawns it when the state exits to `InGame`.

Default overlay: a centred `Label` reading "Loading…" on a solid background.

### Configurable loading scene (optional)
Add `loading_scene: Option<String>` to `ProjectConfig`. When set, the engine loads
that `.scene.ron` as an `Overlay` before entering `LoadingScene` state. The scene
authors the visual however they like (progress label bound to `flycam_position`-style
update, animated mesh, etc.). When absent, the engine default is shown.

### Progress events
Add `scene.loading_progress:{0-100}` events emitted at key milestones:
- `0` — state entered `LoadingScene`
- `25` — scene RON asset loaded
- `50` — all GLB / texture handles issued to the asset server  
- `75` — terrain async task dispatched (if terrain present)
- `100` — `AppState` transitions to `InGame`

These events fire into the normal message pipeline, so a loading scene can bind a
label to them via a rule: `on: scene.loading_progress:75 → UpdateLabel(…)`.

Full byte-accurate progress (`LoadState::Loading` polling per handle) is out of scope
for v1 — the milestone approach is good enough and avoids per-frame handle iteration.

### Terrain progress
Terrain mesh generation runs on `AsyncComputeTaskPool`. When the task completes the
worker can emit `scene.loading_progress:90` before final spawn. This covers the
longest part of the loading pause.

## Tasks
- [ ] Add `LoadingOverlay` marker component
- [ ] `loading_screen_system`: spawn default overlay on enter `LoadingScene` / `LoadingProject`, despawn on `InGame`
- [ ] Emit `scene.loading_progress:{n}` at the five milestones in `scene_loader.rs` and `terrain.rs`
- [ ] Add `loading_scene: Option<String>` to `ProjectConfig` schema
- [ ] Load configured loading scene as overlay before transitioning to `LoadingScene`
- [ ] Tests: verify overlay exists during loading state, absent after InGame
- [ ] Docs: update `30_runtime_events_and_logic.md` with the new events

## Open questions
- Should the default overlay have an animated element (e.g. a spinning dot via `Motion`) or just static text? Static is simpler for v1.
- Does `loading_scene` need to support its own logic (state machine)? Probably not — it's a pure visual, no interactivity needed.

## Acceptance criteria
- Given a project with terrain, when the scene loads, a "Loading…" overlay is visible from the moment the scene starts loading until `InGame` is reached.
- Given `loading_scene: "scenes/splash.scene.ron"` in project config, the splash scene is shown instead of the default text.
- Given `scene.loading_progress:75` fires, a label in the loading scene can be updated by a rule reacting to that event.

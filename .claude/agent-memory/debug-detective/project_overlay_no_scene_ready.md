---
name: overlay-no-scene-ready
description: LoadSceneOverlay/ToggleOverlay never emit scene.ready (only scene.loaded), so "bind initial UI state to scene.ready:<scene>" advice silently never fires for overlay nodes
metadata:
  type: project
---

`spawn_scene_v2` writes `SceneEvent::Ready` only `if !is_overlay` (scene_loader.rs, comment: overlays must not overwrite debug.scene). `SceneEvent::Loaded` fires for overlays too, and since `spawn_scene_v2.before(fsm_interpreter_system)` gets an auto sync point, the overlay's UI entities already exist when a `scene.loaded:<overlay>` binding executes.

**Why:** found reviewing feature/ui_set_visible (2026-10-12): docs told designers to hide UI on `scene.ready:<scene>`, which is a dead hook for overlay nodes; an action in the same list as LoadSceneOverlay is a no-op (overlay spawns next frame at the earliest).
**How to apply:** any feature whose docs say "author initial state on scene.ready" must say `scene.loaded:<overlay>` for overlays; check a runtime test exists for the recommended hook rather than only the action executor.

---
name: project-dynamic-split-screen
description: dynamic_split_screen_system hot-path profile (Stage 5 of local co-op split-screen)
metadata:
  type: project
---

`dynamic_split_screen_system` in camera.rs runs per-frame in the render-cadence `.chain()` between party_camera_follow_system and split_screen_viewport_system (lib.rs).

- Early-returns at top if `DynamicSplitConfig` resource is `None` — free on all non-dynamic scenes (just a Res check, no alloc, no query iter).
- On dynamic scenes: allocates `Vec<Entity>` every frame via `split_cameras.iter().map(|(_,orbit)| orbit.target).collect()` — 2 elems, avoidable but trivial. Nit: read the two targets directly or cache in `Local<[Entity;2]>`.
- Camera mutation (is_active) and ActiveSplitScreen write only happen when hysteresis threshold crossed (split_distance / merge_distance); most frames just do one Vec3::distance + compare.
- 3 permanent camera entities per dynamic scene (1 PartyOrbitCamera + 2 OrbitCamera+SplitViewportSlot), spawned once at scene load, no runtime spawn/despawn. At most 2 active at once — rendering never exceeds Stage 3/4 2-camera cost.

Relates to [[project_split_screen_viewport]], [[project_local_coop_input_camera]]. Zero new deps.

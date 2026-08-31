---
name: label-depth-scale
description: label_depth_scale resolver/validation — spawn-time-only call sites, per-frame-safe; default ref distance 20.0 tied to default_camera_config max_radius
metadata:
  type: project
---

`resolve_label_depth_scale` (`runtime/scene_manager/mod.rs`) is **not** per-frame despite the "hot
per-widget call site" wording in its doc comment. All callers are spawn/scene-load scoped:
`nameplate.rs` setup, the four `scene_loader.rs` scene-load widget spawn loops, and
`drain_dynamic_stat_ui_system`'s drain loop (per-frame system, but the queue is empty on idle
frames so the call count is zero). Its `is_finite()` + `clamp()` are a few float ops per widget —
free at that frequency.

**Why:** reviewed 2026-08-31 (`feature/label-depth-scale-validation`); the doc comment's "hot"
phrasing invites a false per-frame flag on future reviews.

**How to apply:** don't re-flag the clamp. Scene-load `warn_*` diagnostics in `spawn_scene_v2`
(the block after `is_split_screen`) are all one-shot per scene load — new sibling `warn_*` fns
added there inherit that and need no separate frequency analysis.

`default_label_ref_distance()` (`schema/scene_v2.rs`) is deliberately kept equal to
`entity_spawner::default_camera_config()`'s `max_radius` (both `20.0` as of 2026-08-31, changed
from `50.0`). Changing either without the other silently desyncs depth scaling — and changing it
alters widget size in any scene with a `label_depth_scale` block, which moves `test_web.py`
screenshot baselines.

Related: [[project_world_label_screen_pos]], [[project_nameplate_system]],
[[project_stat_widget_spawn_extraction]].

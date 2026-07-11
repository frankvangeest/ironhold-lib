---
name: project-particle-billboard
description: rebuild_pool_meshes_system camera-selection hot path — per-frame filter+min_by_key over ≤4 cameras, read-only &Camera
metadata:
  type: project
---

`rebuild_pool_meshes_system` (crates/ironhold_core/src/capabilities/particle_renderer.rs, ~L292) runs every frame and rebuilds particle billboard vertices.

Camera-selection cost (as of feature/particle-billboard-orientation, ~L311): `camera_q.iter().filter(is_active).min_by_key(camera_priority_key)` over at most `MAX_SPLIT_PLAYERS = 4` Camera3d entities. Done ONCE per frame regardless of particle count — resulting `(cam_right, cam_up)` reused for every particle. `camera_priority_key(entity, slot) -> (u32, Entity)` (camera.rs ~L265) returns a Copy tuple; no allocation, lazy iterator. Negligible for WASM frame budget.

**Why:** shared `camera_priority_key` gives deterministic split-screen camera pick, same order as `world_label_screen_pos_system`.

**How to apply:** The `&Camera` access here is READ-ONLY. Other Update systems take `&mut Camera` (`dynamic_split_screen_system` toggles is_active, `split_screen_viewport_system` writes viewport). On WASM (single-threaded) parallel-scheduling conflicts are moot; Bevy also serializes read vs write of the same component on native — no borrow panic, just no parallelism. One-frame staleness in which camera is picked is cosmetically invisible, so ordering relative to those mutators does not matter for correctness.

**Pre-existing (NOT this diff, out of scope but worth noting):** the same system builds `HashMap<GroupKey, Vec<usize>>` (~L319) and per-group index Vecs every frame — a genuine per-frame allocation, but unchanged by the billboard fix.

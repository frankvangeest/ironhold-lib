---
name: local-coop-hot-join
description: Action::JoinPlayer hot-join into Grid split; executor-arm-only work, no per-frame regression; SpawnParams gained 3 resource fields (near-zero fetch cost)
metadata:
  type: project
---

`Action::JoinPlayer` (action_executor.rs) spawns a new player + one incremental split camera into an already-Grid-split local co-op scene at runtime, up to MAX_SPLIT_PLAYERS=4.

**Why:** Local-coop batch feature (see [[project_local_coop_batch_before_main]] context). Grows split layout live, no scene reload.

**How to apply (hot-path facts for future reviews):**
- The JoinPlayer arm runs ONLY when that action is popped off ActionQueue (one actual join press), NOT per-frame. Its allocations (`format!` spawn_id/spawn_point_key, `prefab_def.clone()`, `model_path.clone()`) are per-join, negligible.
- The `pending_spawns.0.iter().filter(is_hot_join).count()` linear scan is bounded: VecDeque is drained at SPAWNS_PER_FRAME per frame and only grows while actions are actively queued. Scan runs only on a JoinPlayer pop. Safe, not a per-frame O(n).
- `SpawnParams` (bundled SystemParam used every frame by action_executor_system, unconditional in Update) gained `scene_handle: Option<Res<SceneHandleV2>>`, `scenes: Res<Assets<GameSceneV2>>`, `active_split_slot_count: Res<ActiveSplitSlotCount>`. Adding Res fields = resource-storage index + change-tick check per frame; near-zero, not measurable. ECS parallelism cost is irrelevant on single-threaded WASM.
- `ActiveSplitSlotCount(Option<u32>)` is init_resource'd (always present, so bare `Res` is safe); `Some(n)` only while Grid-split, `None` otherwise — used as the hot-joinable scope guard. drain_spawn_queue_system owns the ResMut that increments it.
- entity_spawner.rs: `spawn_split_camera_for_player` is a thin wrapper over pre-existing `spawn_orbit_camera_for_player` (no new allocs). drain_spawn_queue_system gained one is_hot_join branch inside its existing SPAWNS_PER_FRAME-capped loop — per-spawn, not new per-frame cost.
- Zero new dependencies (no Cargo.toml change). No WASM-incompatible API (only format!/warn!/info!). No threads/fs/blocking. Verdict was clean.

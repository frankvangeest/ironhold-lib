---
name: join-prefab-keys-slot-arithmetic
description: join_prefab_keys is an unbounded Vec but only indices in [existing slot_count, MAX_SPLIT_PLAYERS) are ever reachable — slot 0 is dead in any scene that already has a player, and slots >= 4 can never be joined
metadata:
  type: project
---

`GameSceneV2.join_prefab_keys: Vec<Option<String>>` (`scene_v2.rs:148`) has **no length cap**, but
`Action::JoinPlayer` (`action_executor.rs:1676-1700`) indexes it by
`next_slot = active_split_slot_count + queued_hot_joins` and bails with a `warn!` when
`next_slot >= MAX_SPLIT_PLAYERS` (`capabilities/camera.rs:631`, = 4). Consequences:

- **Slot 0 is normally dead.** A Grid-split scene that scene-authors its P1 starts at
  `slot_count == 1`, so the first join reads `join_prefab_keys[1]`. `local_coop_demo/room8` encodes
  this by authoring `[None, None, "player_p3_grid", "player_p4_grid"]` — the `None`s are the
  already-scene-placed players, not unused slots.
- **Slots >= 4 are unreachable.** Any validator/tooling that iterates every `Some` entry and demands
  a matching resource for it will false-positive on slots 4+.
- `player_{next_slot + 1}_start` is 1-based, looked up in `LoadedSpawnPoints` (the current *Replace*
  scene's `spawn_points`), and a miss is **silent** — no `warn!`, just "primary player position +
  1.5 * next_slot on X".
- Only `Grid` split supports hot-join at all (`ActiveSplitSlotCount` is `Some` only for Grid);
  party/dynamic/Vertical/Horizontal/single-camera scenes reject every join. Nothing design-time
  checks that a scene authoring `join_prefab_keys` is actually Grid-split.

**How to apply:** when validating anything derived from a `join_prefab_keys` index, import
`ironhold_core::capabilities::camera::MAX_SPLIT_PLAYERS` and skip (or separately diagnose) slots at
or beyond it rather than hardcoding 4 or ignoring the bound. Related:
[[scenehandle-overlay-never-restored]], [[renderlayers-reserved-scheme]].

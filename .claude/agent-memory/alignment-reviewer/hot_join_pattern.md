---
name: hot-join-pattern
description: Action::JoinPlayer hot-join touchpoints + the 0-based-slot vs 1-based-player_N_start spawn-point off-by-one footgun
metadata:
  type: project
---

`Action::JoinPlayer` (no payload) grows an already-`Grid`-split local-coop scene live, up to
`MAX_SPLIT_PLAYERS` (4). v1 = Grid-only, GLB-players-only, join-only (no leave/gamepad). See
`planning/features/local_coop_hot_join_leave.md`. Related: [[local_coop_pattern]],
[[per_player_stat_pools_pattern]], [[player_model_source_unification_pattern]].

**Touchpoints (all present & correct in v1):**
- `schema/actions.rs` — `Action::JoinPlayer` variant (well-documented).
- `schema/scene_v2.rs` — `join_prefab_keys: Vec<Option<String>>`, `#[serde(default)]`, slot-indexed
  (0-based). Backward-compatible; existing scenes deserialize fine.
- `action_executor.rs` `Action::JoinPlayer` arm — scope guard is `ActiveSplitSlotCount.0` being
  `Some` (Grid-only; `None` for Vertical/Horizontal/party/dynamic/single). Computes
  `next_slot = slot_count + already-queued is_hot_join entries` (same-frame double-join safety),
  reads `join_prefab_keys[next_slot]`, builds config via `assemble_player_config`, overrides
  `PlayerIndex = next_slot`, pushes `QueuedSpawn { is_hot_join: true }`. Emits `coop.lobby_full`
  when the join reaches the cap. Pipeline-correct: does NOT push to ActionQueue; goes through the
  deferred `PendingEntitySpawns` queue (same as Action::Spawn's dynamic-player path).
- `entity_spawner.rs` — `drain_spawn_queue_system` `is_hot_join` branch calls
  `spawn_player_entity_core` (camera-less) + `spawn_split_camera_for_player`, then increments
  `ActiveSplitSlotCount`. `spawn_split_camera_for_player` extracted from the static Grid loop and
  shared by both paths.
- `mod.rs` — `QueuedSpawn.is_hot_join`; `SpawnParams` gains `scene_handle`/`scenes`/
  `active_split_slot_count` (bundled because executor is at Bevy's 16-param ceiling).
- `cli query.rs` — `Action::JoinPlayer => "JoinPlayer"` exhaustive-match arm.
- Trigger path: `scene_key_bindings` → `ui.button_pressed:{trigger}` → rule → `JoinPlayer`. Fully
  designer-reachable, no new input code.

**FOOTGUN — spawn-point slot numbering off-by-one:** the executor builds the spawn-point key as
`format!("player_{}_start", next_slot)` with a **0-based** `next_slot`. But every `local_coop_demo`
scene (and the docs' prose) authors **1-based** `player_1_start`..`player_4_start` keys. Because
`next_slot` for the first joiner into a 2-player scene is `2`, the code fetches `player_2_start` —
which in the demo is player 2's own spot — so the 3rd player spawns on top of player 2, and
`player_4_start` is never reached. The unit test (`local_coop_tests.rs::load_grid_scene_with_join_slots`)
authors 0-based keys, so it passes and masks the demo mismatch. Note: `player_N_start` was never
read by any code before this feature (initial players use their entity `initial_position`), so
there was no pre-existing consumer to match — the 0-based/1-based split is baked in fresh. Fix is a
naming-convention decision (make code 1-based to match the project's authored keys, or re-author the
demo/docs/test to 0-based consistently). When reviewing hot-join or any future `spawn_points`
consumer, always cross-check the index base the code constructs against the base the demo scenes author.

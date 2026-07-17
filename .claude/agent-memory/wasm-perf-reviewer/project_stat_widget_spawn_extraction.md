---
name: stat-widget-spawn-extraction
description: player_stat_widgets refactor — spawn_stat_label_widget/spawn_world_stat_bar_widget extracted to stat_display.rs; all spawn-time; DynamicStatUiQueue is a drained-each-frame Vec (empty=free)
metadata:
  type: project
---

`capabilities/stat_display.rs` gained two pub spawn-time fns `spawn_stat_label_widget` /
`spawn_world_stat_bar_widget` (+ `StatWidgetSpawnCtx`), extracted from previously-duplicated
widget-construction code in `scene_loader.rs` (two spawn loops) and `drain_dynamic_stat_ui_system`.
Player widgets now route through the same `DynamicStatUiQueue` mechanism NPC/prop `Action::Spawn`
uses (producer added in `entity_spawner.rs::spawn_player_entity_core` + the primitive-player path
in `scene_loader.rs`).

**Why:** knowing this is pure spawn-time extraction avoids re-flagging the `format!`/`.clone()`
calls inside those fns as per-frame — they only run on scene-load or dynamic-spawn.

**How to apply:**
- `spawn_scene_v2` (Update, runs every frame) early-returns before touching any of this on a normal
  in-game frame: `if !ready_to_spawn { return; }` then `if InGame && !is_overlay { return; }`.
  Adding `dynamic_stat_ui_queue: ResMut<DynamicStatUiQueue>` to `SceneV2Params` costs nothing at
  idle (early return precedes field use; web is single-threaded so the extra ResMut scheduling
  conflict is irrelevant).
- `drain_dynamic_stat_ui_system` (Update, every frame) drains a `Vec` (`DynamicStatUiQueue.0`) fully
  each frame; empty on idle frames so `for entry in queue.0.drain(..)` is a no-op. Fully drained
  each frame => no unbounded growth. Idle cost = 2 Res reads + bool OR for `is_split_screen`.
- Distinct from ActionQueue: this queue is a plain `Vec`, order irrelevant (widget spawns are
  independent), unlike [[project_actionqueue_lifo]] FIFO constraint.
- Per-frame stat-widget update cost is unchanged by this refactor — see
  [[project_stat_widget_split_duplication]] (format! runs 4x for split ranks) and
  [[project_per_player_stat_pools]]; those update systems were not touched.

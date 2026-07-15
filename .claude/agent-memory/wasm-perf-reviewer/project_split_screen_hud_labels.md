---
name: project-split-screen-hud-labels
description: Split-screen per-player HUD "P{n}" labels (camera.rs) — update system now guards Node.left/top + Visibility writes correctly (earlier unguarded issue fixed)
metadata:
  type: project
---

Split-screen player HUD corner labels, added commit af6727f in `capabilities/camera.rs`. Two Update systems:

- `split_viewport_player_label_spawn_system` — `Added<SplitViewportSlot>`-filtered, fires at most MAX_SPLIT_PLAYERS=4 times total per scene load (per-spawn, not per-frame). Uses `format!` x2 per camera (Name + Text) — negligible per-spawn. Free on non-split scenes.
- `split_viewport_player_label_update_system` — per-frame, `.chain()`ed `.after(split_screen_viewport_system)`. Query `With<SplitScreenPlayerLabel>` on cameras → 0 entities on all non-split-screen scenes, so **free everywhere except local_coop_demo split rooms**.

**RESOLVED (verified 2026-07-13, feature/per-player-split-screen-targeting):** the update system now guards BOTH the `Visibility` write and the `Node.left`/`Node.top` writes with PartialEq (`if node.left != new_left { node.left = new_left; }`), with an explicit comment citing the taffy-relayout reason. The earlier unguarded-Node issue (af6727f) is fixed. This is now the reference-correct viewport-tracking idiom; the new `target_hud_update_system` (see [[project_per_player_targeting]]) copies it faithfully.

Related: [[project_dynamic_labels_system]] (same guard idiom), [[project_split_screen_viewport]], [[project_dynamic_split_screen]], [[project_target_indicator_system]] (epsilon-guarded move done correctly).

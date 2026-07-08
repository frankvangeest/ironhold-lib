---
name: project-split-screen-hud-labels
description: Split-screen per-player HUD "P{n}" labels (camera.rs) — update system has unguarded Node.left/top write violating change-detection discipline
metadata:
  type: project
---

Split-screen player HUD corner labels, added commit af6727f in `capabilities/camera.rs`. Two Update systems:

- `split_viewport_player_label_spawn_system` — `Added<SplitViewportSlot>`-filtered, fires at most MAX_SPLIT_PLAYERS=4 times total per scene load (per-spawn, not per-frame). Uses `format!` x2 per camera (Name + Text) — negligible per-spawn. Free on non-split scenes.
- `split_viewport_player_label_update_system` — per-frame, `.chain()`ed `.after(split_screen_viewport_system)`. Query `With<SplitScreenPlayerLabel>` on cameras → 0 entities on all non-split-screen scenes, so **free everywhere except local_coop_demo split rooms**.

**Known issue (as of af6727f):** in the update system the `Visibility` write IS guarded (`if *visibility != new_visibility`, lines ~440-443) but the `Node.left`/`Node.top` writes are **unconditional** (lines ~451-452):
```rust
node.left = Val::Px(...);
node.top  = Val::Px(...);
```
`node` is `Mut<Node>`, so these mark the Node changed every frame regardless of value, re-triggering Bevy `ui_layout_system` (taffy) relayout every frame on the single-threaded WASM main thread — on the split-screen path that already pays 4x viewport render cost. Positions only actually change on window resize. Violates the documented **change-detection discipline** rule in `crates/ironhold_core/src/CLAUDE.md` (~line 237). Fix: guard with `Val` PartialEq (`if node.left != new_left { node.left = new_left; }`). Cost is modest in absolute terms (<=4, max 2 active in dynamic split) but avoidable and against the stated rule.

Related: [[project_dynamic_labels_system]] (same guard idiom), [[project_split_screen_viewport]], [[project_dynamic_split_screen]], [[project_target_indicator_system]] (epsilon-guarded move done correctly).

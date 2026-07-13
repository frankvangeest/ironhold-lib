---
name: project-stat-widget-split-duplication
description: Phase 4 stat_label/Ascii world_stat_bar WorldLabelRank split-screen duplication — per-frame format! cost in update systems multiplies 4x incl hidden ranks
metadata:
  type: project
---

Phase 4 of `planning/features/split_screen_camera_followups.md` (branch feature/stat-widget-viewport-duplication). Extends `WorldLabelRank` duplication to `stat_label` + Ascii `world_stat_bar` in `scene_loader.rs` (two spawn sites: scene-load `pending_stat_labels`/`pending_world_bars` loops ~L1024-1174, and `drain_dynamic_stat_ui_system` ~L2571). Pixel bars excluded (child hierarchy).

**Gating is the key difference from the world_labels precedent.** world_labels/label duplicate to 4 ranks in EVERY scene; stat widgets gate on `is_split_screen` so single-cam scenes get exactly 1 entity (zero regression). Scene-loader gate: `player_configs.first().is_some_and(|p| p.camera.split.is_some())` (captured before player_configs moved into PendingPlayerConfig). Drain gate: `active_split.0.is_some() || dynamic_split.0.is_some()` (both, because a merged dynamic split reports None on ActiveSplitScreen but DynamicSplitConfig stays Some for scene lifetime). ranks = `if is_split_screen { MAX_SPLIT_PLAYERS=4 } else { 1 }`.

**Why gated (and the real per-frame cost):** unlike static world_labels text, `stat_label_update_system` + `world_stat_bar_update_system` (`capabilities/stat_display.rs`, Update schedule, UNCHANGED by this feature) rewrite every marker's Text2d each frame and do an UNCONDITIONAL `format!` (String alloc) per entity BEFORE the `text.0 != new_text` change-detection guard. So the alloc always happens; only the render-write is guarded. Splitting to 4 ranks = 4x these per-frame `format!` allocs in split scenes — and ranks 1..3 pay it even while `Visibility::Hidden` (update systems don't check Visibility). In a 2-way Vertical/Horizontal split (the common case) only 2 ranks are ever visible, so HALF the rank entities are pure per-frame `format!` waste. Matches world_labels precedent (always 4) so consistent, but a real minor inefficiency. Non-blocking because gated to split scenes only (the deliberate 4x-cost path). Possible future opt: spawn actual-slot-count ranks instead of MAX_SPLIT_PLAYERS, and/or skip Hidden instances in the update systems.

**Throttling (Q asked): adequate, transitive.** `DynamicStatUiQueue`'s ONLY producer is `drain_spawn_queue_system` (entity_spawner.rs), capped at `SPAWNS_PER_FRAME=2`. It pushes ≤2 entries/frame; `drain_dynamic_stat_ui_system` runs right after in the same `.chain()` and drains fully — but queue never holds >2. So worst-case dynamic per-frame spawn = 2 entries × (4 label + 4 bg + 4 fill) = ≤24 Text2d entities/frame. Text2d shares one 2D pipeline → NO new WebGPU pipeline compile, no stall. No separate cap needed on drain_dynamic. No unbounded growth.

**Binary size:** zero new deps, zero new assets. `WorldLabelRank(u8)` pre-existed. Two new `Res` params + loop code = negligible.

**bg_chars clone (Q asked): negligible, dismiss.** `" ".repeat(cells_clamped)` computed ONCE outside the rank loop; `.clone()` per rank (≤4) is scene-load/per-spawn only, not per-frame. Not worth flagging.

Related: [[world-label-screen-pos-system]], [[project-split-screen-hud-labels]].

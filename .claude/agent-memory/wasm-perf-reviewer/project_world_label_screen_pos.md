---
name: world-label-screen-pos-system
description: world_label_screen_pos_system per-frame cost — viewport-aware camera Vec (<=4), O(N) not quadratic, Local reuse not viable
metadata:
  type: project
---

`world_label_screen_pos_system` (`crates/ironhold_core/src/lib.rs`, ~L507) runs every Update. Projects each `WorldLabel` through the active `Camera3d` whose viewport rect contains it (viewport-aware fix for split-screen; replaced old `.single()` that early-returned on 2+ cameras).

Per-frame cost profile:
- Collects `camera.is_active` cameras into a `Vec` then `sort_by_key`. Vec holds **borrowed** tuples `(Entity, &Camera, &GlobalTransform, Option<&SplitViewportSlot>)` — size <=4 (MAX_SPLIT_PLAYERS=4), 1 in normal scenes. One tiny malloc/frame.
- Per label: selection is now `filter_map(...).nth(rank)` (was `find_map`, first hit) over the <=4-camera Vec (`world_to_viewport` + `logical_viewport_rect` + `rect.contains`). Lazy iterator, no alloc, short-circuits at rank-th qualifying camera. Total O(N labels × M cameras), M<=4 → linear, **no quadratic path**. M=1 in single-cam scenes = zero regression.

**2026-07-10 amendment (WorldLabelRank):** `runtime/scene_manager/mod.rs` added `WorldLabelRank(pub u8)`; rank absent == rank 0. Two spawn sites in `scene_loader.rs` now spawn MAX_SPLIT_PLAYERS(4) sibling entities per authored label (ranks 0..3) so a point visible in 2+ split viewports shows in each: the scene-level `world_labels:` loop, **and** the per-entity `label:`/`EntityLabelDef` (`pending_labels`) loop — the latter was missed on the first pass (perf-reviewed only the `world_labels:` site) and is actually the one `local_coop_demo`'s portal room-name labels use in practice (`tracked_entity`, not fixed `world_pos`). Nameplates/damage-popups/stat labels unchanged (single-instance, implicit rank 0). Cost: 4x entities + 4x per-frame projection **scoped to these two label-producing sites only** — one-time 4x spawn at scene load (4x Text2d + text String clone, negligible), per-frame bounded at <=16 projection calls/authored label. Reviewed OK for web. `WorldLabelRank(u8)` is zero-size-impact, no new deps.

**Why:** the natural allocation-avoidance instinct (hoist to `Local<Vec<_>>`) does NOT apply here — the Vec stores references borrowed from the query, which cannot outlive the system call, so a `Local` can't hold them across frames. Would require storing owned `Entity` ids + re-fetching, more complex than the <=4-elem alloc it saves.

**How to apply:** treat the per-frame <=4 Vec+sort as negligible; do not recommend `Local` reuse for it. Transform/font writes are already change-detection guarded (>=0.5px). Zero new deps. Related: [[project_split_screen_hud_labels]].

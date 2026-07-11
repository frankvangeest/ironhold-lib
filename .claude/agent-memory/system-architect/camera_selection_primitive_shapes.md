---
name: camera-selection-primitive-shapes
description: The split-screen "which Camera3d" selection is three DISTINCT shapes across sites, not one reusable primitive; plus the WorldLabelRank 4x-duplication common-case cost trap for per-frame-updated consumers
metadata:
  type: project
---

The reference `world_label_screen_pos_system` (`lib.rs`) camera-selection primitive is NOT uniformly reusable across the four single-camera-assumption sites. There are three genuinely different selection shapes:

- **world_label / nameplate distance-cull (v1)** — project a WORLD point via `camera.world_to_viewport`, test `logical_viewport_rect().contains()`, pick first in sorted order. v1 MUST be bit-identical to world_label (same sort tie-break: `SplitViewportSlot` index then `Entity`), because both must agree on which camera is authoritative for a given nameplate anchor — the anchor is rank-0/single-instance, so a mismatch = the exact flicker v1 claims to prevent. This is the strongest case for extracting a shared helper (or storing world_label's chosen camera/distance on the WorldLabel and reading it), NOT reimplementing.
- **particle billboard basis (v3)** — NO point to project; only reuses the `is_active` filter + sorted-first-camera. Cannot use the containment test at all. Sharing with v1/v4 is limited to the sort comparator.
- **click-to-select (v4)** — the CURSOR is already screen-space, so no `world_to_viewport` projection for selection; just `logical_viewport_rect().contains(cursor)`. Then uses the selected camera's `world_to_viewport` for candidate distance math.

Common denominator worth extracting = the sorted-active-cameras comparator (correctness-critical, most likely to silently drift) and optionally a `camera_for_screen_point(point)` helper (v4 calls with cursor, v1 calls after projecting a world point). v3 only needs `first_active_camera_sorted()`.

**WorldLabelRank 4x-duplication cost trap:** the shipped `world_labels:`/`label:` fix spawns MAX_SPLIT_PLAYERS(4) ranked siblings, ranks 1-3 start `Visibility::Hidden`. For STATIC text (world/entity labels) this is nearly free. Extending it to stat labels / world stat bars (proposed v2) is NOT free: `stat_label_update_system`/`world_stat_bar_update_system` query by marker and rewrite Text2d every frame regardless of Visibility, so 4 ranked siblings = 4x per-frame text-layout/glyph work — including in single-camera scenes where ranks 1-3 are permanently Hidden. That's a common-case regression, not just a split-scene cost. Mitigations: conditionally spawn ranks only when the scene is actually split-screen, or gate the update systems on `Visibility::Visible`.

**Dynamic-spawn divergence:** stat labels + Ascii bars have TWO independent spawn sites — the scene-loader `pending_stat_labels`/`pending_world_bars` loops AND `drain_dynamic_stat_ui_system` (~scene_loader.rs:2543, the `Action::Spawn` wave-spawn path). Any rank-duplication change must cover both or wave-spawned enemies diverge from scene-placed ones (same class as the tag_spawned_entity / four-player-site centralization bugs). See [[world_space_widgets]], [[split_screen_and_shared_mouse]], [[camera_architecture]].

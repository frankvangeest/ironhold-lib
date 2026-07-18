---
name: world-label-stat-ui-pattern
description: How stat_label/world_stat_bar (and damage popups) reach scene-placed vs dynamically-spawned entities, and why depth_scale:None on dynamic spawns is an accepted limitation not a misalignment
metadata:
  type: project
---

`PrefabDef.stat_label` and `PrefabDef.world_stat_bar` are designer-authored RON fields that
spawn `WorldLabel`-tracked Text2d / Mesh2d widgets following an entity. Two spawn routes exist
and they are DIFFERENT code (do not assume one covers the other):

1. **Scene-placed entities** — `spawn_scene_v2` collects `pending_stat_labels` /
   `pending_world_bars` and spawns the widgets inline (scene_loader.rs ~line 1040+). These DO
   resolve depth scaling via `resolve_label_depth_scale(scene.label_depth_scale, per_label)`.

2. **Dynamic `Action::Spawn` entities** — `drain_spawn_queue_system` (entity_spawner.rs) pushes a
   `DynamicStatUiEntry` (entity + pre-resolved stat keys) onto `DynamicStatUiQueue`
   (mod.rs). `drain_dynamic_stat_ui_system` (scene_loader.rs ~line 1760) drains it next frame and
   spawns the same widget set. Registered in lib.rs after `drain_spawn_queue_system` in the
   chained Update set. `Action::Spawn` is the SOLE dynamic-spawn entry (always queues to
   `PendingEntitySpawns`, drained only by `drain_spawn_queue_system`), so this covers every
   dynamically-spawned prefab regardless of kind (GLB/primitive/composite).

**Dynamic-spawn stat widgets NOW inherit scene depth scaling (shipped on branch
`feature/depth-scale-dynamic-spawn`, ~2026-07-11).** New resource
`LoadedLabelDepthScale(Option<LabelDepthScaleDef>)` (mod.rs) is populated in `spawn_scene_v2`
(scene_loader.rs:687, right beside `ActiveTonemapping`) and read by `drain_dynamic_stat_ui_system`
(scene_loader.rs:2543) which now calls `resolve_label_depth_scale(label_depth_scale.0.as_ref(),
None)` — byte-identical to the scene-placed call sites (scene_loader.rs:1022/:1038). So a
wave-spawned enemy's stat label/bar shrinks with distance exactly like a scene-placed one. This
supersedes the older "depth_scale: None on dynamic spawns is accepted" note. Chosen mechanism was a
scene-load-time resource (mirrors ActiveTonemapping), NOT threading a resolved tuple through
`DynamicStatUiEntry` as I'd earlier guessed — the resource lets the dynamic path call the *same*
helper rather than duplicating precedence logic.

**SPLIT-SCREEN RANK DUPLICATION (Phase 4 of split_screen_camera_followups, reviewed 2026-07-12,
ALIGNED):** `stat_label` + Ascii `world_stat_bar` now spawn `MAX_SPLIT_PLAYERS` (4) `WorldLabelRank`
sibling entities in split-screen scenes (rank 0 visible primary + ranks 1..3 `Visibility::Hidden`
until `world_label_screen_pos_system` resolves a camera), matching the scene-`world_labels:` pattern.
Non-split scenes still spawn exactly 1 (no rank component) — zero behavior change. Gate:
- Scene-load path (both spawn loops): `let is_split_screen = player_configs.first().is_some_and(|p|
  p.camera.split.is_some())` captured at scene_loader.rs:693 BEFORE player_configs is moved into
  PendingPlayerConfig (terrain-delayed path). `.split.is_some()` alone is correct here — at scene
  load `split` is always Some for fixed/dynamic/Grid split scenes.
- Dynamic `Action::Spawn` path (`drain_dynamic_stat_ui_system`): ORs `ActiveSplitScreen.0.is_some()
  || DynamicSplitConfig.0.is_some()` — MUST check both, because a dynamic split reports
  `ActiveSplitScreen(None)` while merged even though 2 real cameras are alive; DynamicSplitConfig
  stays Some for the scene lifetime.
Safe because `stat_label_update_system`/`world_stat_bar_update_system` rewrite EVERY instance's
Text2d each frame with no Visibility gate (verified: neither query filters on Visibility) — unlike
the static world_labels text, so hidden ranks stay current. No ActionQueue, no schema, gate derived
from existing `camera.split`.

**PIXEL BARS NOW DUPLICATE TOO (reviewed 2026-07-17, `feature/pixel-world-stat-bar-split-screen-duplication`,
ALIGNED).** `spawn_world_stat_bar_widget`'s `Pixel` arm (stat_display.rs) now wraps its whole
anchor+children hierarchy in `for rank in 0..ranks` (`ranks = if is_split_screen {MAX_SPLIT_PLAYERS}
else {1}`), same gate as Ascii. Border/bg mesh+material registered ONCE and `.clone()`d per rank
(handle clone = cheap Arc, identical geometry); fill mesh/material created fresh per rank (each
updated independently by `world_pixel_bar_update_system`). Anchor carries the `WorldLabelRank`;
children carry none (Bevy `InheritedVisibility` cascades the anchor's `Visibility::Hidden`). Pure
runtime fix, NO schema change — a designer writes `style: Pixel(...)` and gets correct split-screen
behavior with zero new fields. Pixel bars still get NO depth scaling (anchor `depth_scale: None`,
pre-existing documented exclusion). local_coop_demo player_p1_split/p2_split switched Ascii→Pixel as
the playtest aid. Tests in local_coop_tests.rs assert fill COUNT scales to MAX_SPLIT_PLAYERS, anchor
rank identity 0-3, AND a regression guard that mesh/mat counts == `2 + MAX_SPLIT_PLAYERS` (proves
border/bg shared, only fills scale). Nameplate anchors + damage popups remain single-instance
(only remaining rank-0-only consumers).

STALE-DOC WATCH: mod.rs WorldLabelRank doc (~348-352) and src/CLAUDE.md WERE updated for Pixel. BUT
`lib.rs` `world_label_screen_pos_system` doc (lines 508-509 + 514-516) was MISSED — still lists
`Pixel`-style world stat bars among "always bind to rank 0" single-instance consumers AND omits
Pixel from the "spawn ranked siblings only in split-screen" list. Factually wrong after this fix;
flagged as a warning at review. (Original Phase-4 flag mentioned lib.rs:505-513 + src/CLAUDE.md:418-422
still listing STAT LABELS as single-instance — those were fixed then; the Pixel-specific lib.rs
staleness is new.)

NOTE — there is NO per-widget `depth_scale` override on `StatLabelDef`/`WorldStatBarDef` (both are
`deny_unknown_fields`); `per_label` is hardcoded `None` at every stat-widget call site. Only
`WorldLabelDef`/`EntityLabelDef` carry `depth_scale: Option<bool>`. Stat widgets purely inherit the
scene block. `style: Pixel` bars remain excluded (anchor `depth_scale: None`) on both paths —
pre-existing documented limitation.

The still-`None` precedent for *transient* widgets stands: `ShowDamagePopup`/`ShowFloatingText`
in action_executor.rs deliberately keep `depth_scale: None` and were correctly left untouched by
this fix (scope guard in the plan).

**THIRD STYLE `WorldStatBarStyle::Icon` ADDED (reviewed 2026-07-18,
`feature/world-icon-stat-bar`, ALIGNED).** Row of per-cell `Sprite` icons (hearts/pips) — each
cell shows `filled_index` or `empty_index` from a designer-authored atlas, reusing the
`icon_sheet`/`icon_cols`/`icon_rows`/`icon_cell_size` convention `ActionBarDef`/`ItemDef` use. New
`WorldIconBar` anchor component + `world_icon_bar_update_system` (resolves stat ONCE per anchor,
walks `&Children` in spawn order to set each `Sprite`'s atlas index — no per-cell marker; relies on
cell spawn order 0..cells). Fill count is `ceil`-based (`filled = max(1, ceil(ratio*cells))`, 0 only
at exactly `ratio==0.0`) — deliberately different from Ascii/Pixel's `round`, documented in the plan.
Texture resolved via `asset_catalog.textures.get(icon_sheet).map(load).unwrap_or_default()` (no
fabricated path); atlas built at runtime from `TextureAtlasLayout::from_grid(icon_cell_size,
icon_cols, icon_rows)` — the companion `.json` sidecar files next to iconsheet PNGs are docs-only,
NOT consumed by the engine or referenced in assets.ron. Pure cosmetic (no ActionQueue). Rank-
duplicates in split-screen like Ascii/Pixel (`for rank in 0..ranks`); anchor `depth_scale: None`
(same pre-existing exclusion as Pixel). `StatWidgetSpawnCtx` gained THREE `Option` fields
(`atlas_layouts`/`asset_server`/`asset_catalog`) — only touched by the Icon arm, `.expect()` if an
Icon bar spawns without them; both `spawn_world_stat_bar_widget` call sites (scene-load ~1083 +
`drain_dynamic_stat_ui_system` ~2663) pass `Some(...)`, stat_label sites pass `None`.
Two footguns flagged (both non-blocking, both consistent with existing precedent so accepted): (1)
typo'd `icon_sheet` key → `unwrap_or_default()` blank Handle with NO spawn-time warn (same as
IconButton; runtime warn only fires for a missing *stat*, not a missing *texture*); (2) CLI
`validate.rs` does NOT cross-check `icon_sheet` against `assets.ron` textures — but it doesn't for
`ActionBarDef`/`ItemDef` `icon_sheet` either (only foliage `leaf_texture` at validate.rs:387 gets
that treatment), so Icon is merely consistent, not a regression.

**Global-vs-`{self}` stat_key on a PLAYER prefab is a documented first-class choice, NOT a
shortcut.** 3rd_person_game_demo's player_male/player_female use `stat_key: "player_health"` (a
GLOBAL `stats/stats.ron`/`LoadedStats` key) on their Icon bars, not `{self}.health`. This is
correct: `resolve_stat` routes any non-dotted key to `LoadedStats` regardless of the attached
entity, and `WorldStatBarDef.stat_key`/`StatLabelDef.stat_key` docstrings explicitly list a global
key as a valid form. This project has no `health` `stat_template` on the player, so `{self}.health`
would resolve empty AND trip the player-stat-widgets Part C warn/CLI guard — global is the only
correct form here. When reviewing a world_stat_bar/stat_label on a player, do NOT flag a global key
as a `{self}` mismatch; check whether the project tracks that stat globally or per-entity first.

STALE DOC WATCH: the doc-comment block above `drain_dynamic_stat_ui_system`
(scene_loader.rs:2534-2538) still reads "Dynamic spawns never have a label_depth_scale scene config
so depth scaling is always None here" — factually wrong after this fix, and garbled (a stray
"Resolves the effective depth-scale config for a single label." line that belongs to the helper at
:2683 got left above the system). Flagged as a warning at review time.

**Known duplication NOW RESOLVED (2026-07-17, `feature/player-stat-widgets` — see
[[player-stat-widgets-pattern]]).** The triplicated Ascii/Pixel widget-spawn blocks were extracted
into `spawn_stat_label_widget`/`spawn_world_stat_bar_widget` (both `pub` in
`capabilities/stat_display.rs`), taking a `StatWidgetSpawnCtx { meshes, color_materials,
depth_scale, is_split_screen }`. All three call sites (both Phase-B loops + `drain_dynamic_stat_ui_system`)
now call the helpers; each site still resolves its own `depth_scale`/`is_split_screen` beforehand
(scene path: `scene.label_depth_scale` + captured `is_split_screen`; dynamic path:
`LoadedLabelDepthScale` + `active_split.0.is_some() || dynamic_split.0.is_some()`). A new
`WorldStatBarStyle` variant or widget knob now only needs the ONE helper touched. If reviewing a
change here, verify the two depth_scale/is_split_screen sources still differ per-site (they must).

Motion has the parallel structure: see [[prefab_marker_three_spawn_paths]] — `motion` is inserted
in `spawn_prefab_instance` (covers GLB actors + all dynamic spawns) AND separately in the
single-mesh (scene_loader.rs ~401) and composite (~521) primitive branches that don't call
`spawn_prefab_instance`. Removing the GLB-actor inline motion block while keeping the two primitive
blocks is CORRECT — it dedupes the GLB path without breaking primitives.

---
name: project_world_stat_bar_style_landscape
description: world_stat_bar has ONLY Ascii+Pixel styles; Icon is unbuilt vaporware; both styles now duplicate in split-screen
metadata:
  type: project
---

`WorldStatBarStyle` (schema/catalog.rs) has exactly two variants: `Ascii` (text) and `Pixel` (mesh quads). Default is `Ascii` when `style` omitted.

**No `Icon` world_stat_bar style exists.** `Icon` only exists for `IconButton`/inventory atlas widgets. A planned `Icon` stat-bar style lives in `planning/features/world_icon_stat_bar.md` (not implemented). deny_unknown_fields would reject `style: Icon(...)`.

As of feature/pixel-world-stat-bar-split-screen-duplication (2026-07-17): BOTH `Ascii` and `Pixel` `world_stat_bar` now duplicate correctly across split viewports (Pixel previously did not). Damage popups and nameplates remain single-instance (show in at most one viewport). See [[project_depth_scale_field_scope]] for the related no-depth-scale caveat on Pixel bars.

**Why:** docs/20_data_formats.md's soft-deprecation callout listed `Icon` alongside `Pixel` as a "production-quality style," which presents an unavailable option as available.
**How to apply:** when reviewing world_stat_bar docs/examples, flag any use of `style: Icon(...)` as invalid, and any doc text implying Icon is a currently-usable style. Canonical Pixel split-screen example: local_coop_demo player_p1_split/player_p2_split.

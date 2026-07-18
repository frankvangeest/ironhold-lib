---
name: project_world_stat_bar_style_landscape
description: world_stat_bar now has THREE styles Ascii+Pixel+Icon (Icon shipped 2026-07-18); all three duplicate in split-screen
metadata:
  type: project
---

`WorldStatBarStyle` (schema/catalog.rs) has THREE variants: `Ascii` (text), `Pixel` (mesh quads), and `Icon` (row of per-cell `Sprite` icons, e.g. hearts). Default is `Ascii` when `style` omitted.

**`Icon` style shipped in feature/world-icon-stat-bar (2026-07-18)** — previously vaporware. Icon fields: `icon_sheet` (catalog texture key, required), `icon_cols`/`icon_rows` (atlas grid, default 8×8), `icon_cell_size` (default 64), `filled_index`/`empty_index` (row-major, required), `cells` (default 5), `spacing` (default 4, EDGE-to-edge gap like `ActionBarDef.slot_gap`, NOT centre-to-centre — deliberate deviation from the plan), `size` (default 24×24). Fill rounding is `ceil` not round: `filled=0` only at exactly ratio 0; else `max(1, ceil(ratio*cells))` — so 1% shows ≥1 cell, >80% shows full on a 5-cell bar; documented as expected, no partial/half-cell rendering. `color_bands` and `bg_color` are explicitly ignored by Icon; `fill_color`'s Icon disposition is NOT documented (open gap — does it tint sprites or not?).

Canonical Icon example: `3rd_person_game_demo` player_male/player_female (5-heart bar tracking GLOBAL `player_health`). Atlas: `assets/shared/ui/iconsheet-hearts-01.png` (128×64, 2×1 grid, placeholder-but-real art). Docs: 20_data_formats "World-space stat widgets" section, `WorldStatBarStyle::Icon fields` table.

As of feature/pixel-world-stat-bar-split-screen-duplication (2026-07-17) + world-icon (2026-07-18): all three `world_stat_bar` styles duplicate correctly across split viewports. Damage popups and nameplates remain single-instance (show in at most one viewport). See [[project_depth_scale_field_scope]] for the no-depth-scale caveat (Pixel AND Icon never depth-scale).

**How to apply:** `style: Icon(...)` is now valid. When reviewing Icon usage, watch for: (1) `icon_cols`/`icon_rows` defaulting to 8×8 silently producing wrong UVs on a non-8×8 sheet (no error); (2) whether `fill_color` is claimed to tint Icon hearts (undocumented). Canonical Pixel split-screen example: local_coop_demo player_p1_split/player_p2_split.

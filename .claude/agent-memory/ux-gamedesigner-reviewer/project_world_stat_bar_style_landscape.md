---
name: project_world_stat_bar_style_landscape
description: world_stat_bar now has FOUR styles Ascii+Pixel+Icon+Textured (Textured added 2026-07-18); all duplicate in split-screen
metadata:
  type: project
---

`WorldStatBarStyle` (schema/catalog.rs) has FOUR variants: `Ascii` (text), `Pixel` (mesh quads), `Icon` (per-cell `Sprite` row, e.g. hearts), and `Textured` (9-sliced continuous fill, added feature/world-textured-stat-bar 2026-07-18). Default is `Ascii` when `style` omitted. Ascii is the prototyping/debug style; Pixel/Icon/Textured are the production choices.

**`Textured` style (feature/world-textured-stat-bar, 2026-07-18):** two overlapping `Sprite` layers (fill drawn over empty/track) both cropped via `Sprite.rect` from ONE shared sheet (`texture_sheet` catalog key). Fill width driven continuously by stat ratio via `Sprite.custom_size.x`; caps stay undistorted via Bevy `SpriteImageMode::Sliced` (9-slice). Fields: `texture_sheet` (required), `fill_rect`/`empty_rect` `(x,y,w,h)` in TEXTURE px (required), `size` `(w,h)` in SCREEN px (default 64x12), `slice_border` `(left,right,top,bottom)` in TEXTURE px (default 6,6,6,6). Colour: `fill_color`/`color_bands` multiply-tint the fill layer (same selection logic as Pixel); `bg_color` multiply-tints the track layer ONCE at spawn (never animates). White `(1,1,1,1)` = no tint (for pre-coloured art). No depth scaling in v1 (like Pixel/Icon).

Canonical Textured example: `3rd_person_game_demo` player_male/player_female (tracks GLOBAL `player_health`; replaced the Icon hearts bar). Sheet: `assets/shared/ui/rounded-healthbar-texture-sheet.png` (48x48; rows 0-16 solid fill pill, 17-31 hollow track, 32-47 unused padding; colourless mid-grey to be tinted). Docs: 20_data_formats "WorldStatBarStyle::Textured fields" table + 4 callouts (two-pixel-spaces / one-sheet-two-frames / colour-tinting / low-fill).

**Textured designer-facing doc gaps (found in review 2026-07-18, may be fixed later):**
1. **Coordinate origin for fill_rect/empty_rect is UNDOCUMENTED** — Bevy `Sprite.rect` is top-left origin, Y-down (shipped example: fill at y=0 top, empty at y=17 below). Never stated. A designer authoring own art who assumes bottom-left/UV convention crops the wrong region with NO error. Biggest blocker for the "author your own sheet" story.
2. **No image-editor workflow** — docs explain WHAT fill_rect is but never say "open your sheet in an image editor, read off top-left (x,y) + w/h in pixels." Designers have no CLI (Rust binary), only an image editor + WASM build.
3. **Stale summary at 20_data_formats.md line ~1724** — WorldStatBarDef component-table row still lists "Ascii, Pixel, or Icon" (omits Textured); only the detailed section lists four.
4. **Default tints are non-neutral footgun** — default `fill_color` bright green + default `bg_color` dark red-brown will multiply-tint a neutral/pre-coloured sheet unexpectedly; white-tint escape hatch is documented but the "defaults are coloured, not white" gotcha isn't sharp.
5. **"9-slice" term never defined** for non-programmers in the Textured section.
6. **Shipped example over-slices the empty frame** — slice_border (8,8,8,8) vertical insets sum to 16 > empty_rect height 15 (fill_rect height 17 is fine at 16<17). Two frames have different heights (17 vs 15) sharing one slice_border. Verify visually; benign per low-fill note but a smell to inherit when copied.

As of feature/world-textured-stat-bar: all FOUR styles duplicate correctly across split viewports. Damage popups and nameplates remain single-instance. See [[project_depth_scale_field_scope]] (Pixel/Icon/Textured never depth-scale). Color-tuple arity: fill_color/bg_color/color_bands are all 4-tuple RGBA — see [[project_color_tuple_inconsistency]].

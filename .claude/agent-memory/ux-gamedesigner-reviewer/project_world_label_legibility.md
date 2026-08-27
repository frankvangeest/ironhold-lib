---
name: world-label-legibility
description: Why side-by-side gallery-demo entity `label:` captions overlap — fixed screen-px font vs px-per-metre that depends only on viewport HEIGHT; no wrap, no align, centered anchor; camera-back makes it WORSE; the shipped short-token + legend house pattern
metadata:
  type: project
---

Entity `label:` / scene `world_labels:` are `Text2d` entities repositioned each frame by
`world_label_screen_pos_system` (`crates/ironhold_core/src/lib.rs`). Mechanics that decide whether
a row of side-by-side captions is legible — none of them documented in `docs/20_data_formats.md`:

- **Font size is screen pixels, fixed** (`font_size` default `18.0`). It does NOT shrink with
  distance unless the scene sets `label_depth_scale:` (or the label sets `depth_scale: true`).
- **There is no `TextBounds` anywhere in the codebase** → labels **never wrap and never clip**.
  A long line renders as one unbroken run at full width. "Cap the label width so it wraps" is
  **new engine work**, not authorable today. `\n` in `text:` is the only line-breaking tool.
- **No `align` field on `EntityLabelDef`/`WorldLabelDef`** — `Text2d`'s default anchor is centred,
  so each caption spreads ±half its width around the entity's screen X. Multi-line blocks are
  left-justified inside that centred block, so block width = the longest line.
- **Flycam gets Bevy's default 45° vertical FOV** — the flycam spawn path in `scene_loader.rs`
  never calls `insert_fov` (only orbit/split/first-person cameras do, from `CameraDef.fov`
  default 60 / first-person 90). Consequence: horizontal **pixels-per-metre at the specimen plane
  depends only on viewport HEIGHT**, not width:
  `px_per_m = H / (2 · d · tan 22.5°) ≈ H / (0.828 · d)`.
- **Therefore: pulling the camera BACK makes caption overlap WORSE**, not better — the pixel gap
  between specimens shrinks as `1/d` while the text stays the same pixel size. This is the
  opposite of the usual intuition; correct fixes are shorter text, or `label_depth_scale` with
  `reference_distance ≈ the camera's actual distance` (which makes the text/gap ratio
  distance-invariant), or moving the camera CLOSER.
- **A label whose projected point falls outside the viewport rect is hidden entirely**
  (`rect.contains(vp)` in `world_label_screen_pos_system`) — a specimen just past the frustum edge
  loses its caption completely rather than clipping. Since the half-width in metres is
  `d · 0.4142 · aspect`, the outermost specimen in a row pops in and out with window aspect.
- **Quick overlap test:** `pixel_gap = pitch_m · H / (0.828 · d)` vs
  `text_px ≈ chars · font_size · 0.52`. A 50-char caption at 18px is ~470px — needs `H > ~780`
  at 4 m pitch / d = 8. Dimension for the smallest plausible canvas height (~600 on a 1366×768
  laptop), not for a maximised 1080p window.

**Shipped house pattern for labelled galleries — short token on the model, detail elsewhere:**
- `custom_materials/scenes/main.scene.ron` — ~40 specimens, every `label:` is 1–2 words
  ("Unlit Pink", "Checker UV"), plus `label_depth_scale: (reference_distance: 80, min_scale: 0.25)`.
- `particles_demo/scenes/main.scene.ron` — two-tier: a bold 20px name `world_labels:` entry plus a
  dim 14px sub-caption entry ~0.5 m lower. Two separate entries with different `font_size`/`color`,
  **not** a `\n` inside one label.
- `primitive_world/scenes/map.scene.ron` — precedent for a hand-composed screen-space diagram from
  `Rect` + `Label` with `absolute: true`; declare the `Rect` backdrop **before** the `Label`s.
- 8 of ~14 projects set scene-level `label_depth_scale:` (custom_materials, particles_demo ×2,
  effect_mayhem_demo, primitive_world, stats_demo, 3rd_person_game_demo, local_coop_demo rooms
  3/9/10). A new diorama demo that omits it is the outlier — flag it.

**Adjacent authoring limits that shape any fix:**
- `LogicRule` is only `on` + `when` (logic-state guard) + `do_actions` — **no value-based
  conditions**. So any "page N of M" / indexed navigation must be one interpreter state per page
  (or one scene per page); `IncrementVariable` alone cannot drive branching.
- `ButtonDef` has **no `bind`** — button text is static. Only `LabelDef` has `bind` + `format`.
- UI nodes have `position` in absolute top-left pixels only — **no anchor, no percentages**. A
  bottom- or right-anchored panel is not authorable; only a top-left ladder is viewport-robust.
  `ui_panel:` is *centred*, so it can't be used as a side legend over a diorama.
- `SetEntityVisible` **does** auto-hide a tracked entity's per-entity `label:` (the
  `tracked_vis == Hidden` early-return), but `docs/20_data_formats.md` line ~3738 and `STATUS.md`
  only claim "stat bar, stat label" auto-hide. Doc understates it.

**Doc gap:** `docs/20_data_formats.md` has **no field table for `EntityLabelDef` or
`WorldLabelDef`** — only the "Label depth scaling" section (~356–432). `text`, `offset`
(default `(0,7,0)`, badly wrong for character-scale scenes), `font_size` (18), `color` are
undocumented, and nothing warns about horizontal collision between neighbouring captions or the
no-wrap behaviour. No shipped project sets `font_size` on an entity `label:` (only on
`world_labels:`), so that path is schema-supported but untested in assets.

**How to apply:** on any new side-by-side gallery/diorama demo review, run the overlap arithmetic
above at H≈600 before accepting the layout, check `label_depth_scale` is present with
`reference_distance ≈ camera distance`, and push RON-snippet-length captions into a screen-space
legend keyed by a 2–4 character on-model token. Related:
[[dynamic-animation-control-demo]], [[depth_scale-field-scope]], [[screen_offset-stacking-pattern]],
[[local-coop-demo-room-conventions]].

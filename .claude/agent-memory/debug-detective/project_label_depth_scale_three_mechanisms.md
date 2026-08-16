---
name: label-depth-scale-three-mechanisms
description: label_depth_scale is one all-or-nothing scene block fanning out to two dissimilar mechanisms (font-size rasterize vs. anchor Transform.scale resample); adding it changes every world widget in the scene at once, and screen_offset stacking is calibrated to a 720px viewport
metadata:
  type: project
---

`GameSceneV2.label_depth_scale` is a single scene-level block with **no per-widget opt-out** for stat
widgets or nameplates (only `world_labels:` / entity `label:` have `depth_scale: Option<bool>`). Adding
it to a scene silently enables depth scaling for *every* world-space widget in that scene, and the
widgets then diverge into two unrelated implementations:

1. **Font-size path** — `WorldLabel` entities that carry `TextFont` (`world_labels`, entity `label`,
   `stat_label`, `Ascii` `world_stat_bar`). `world_label_screen_pos_system` scales
   `TextFont.font_size` and `.round()`s it — glyphs are **re-rasterized**, so they stay crisp, but at a
   `min_scale` of 0.5 a `font_size: 14` label becomes `7`, near-illegible at 720p.
2. **Anchor `Transform.scale` path** — `WorldLabel` entities with no `TextFont` (nameplate anchors,
   `Pixel`/`Icon`/`Textured` bar anchors). The whole child subtree scales (XY only, Z left alone).
   `Text2d` children are **resampled, not re-rasterized**, so a nameplate name at 0.5 looks softer than
   a `stat_label` rendered at the same effective size right next to it.

The branch discriminator is implicit: "does this entity happen to carry `TextFont`". A future widget
that puts its `Text2d` directly on the anchor would silently switch paths with no compile error.

**History:** `Pixel`/`Icon`/`Textured` bar anchors used to be a third path — hardcoded `depth_scale:
None`, i.e. never scaled. `feature/nameplate-zoom-spacing` (2026-08) removed that exclusion and also
gave nameplate anchors the scene setting (they were hardcoded `None` too). Damage popups /
`ShowFloatingText` (`action_executor.rs`) are the one place still hardcoding `depth_scale: None`, so
they stay full-size while everything around them shrinks.

**`screen_offset` (`StatLabelDef`/`WorldStatBarDef`)** — pixel-space offset added after projection,
multiplied by the same depth-scale factor. Because the factor is `ref_dist/d` inside the working band,
`screen_offset * factor` exactly reproduces the old perspective-projected world gap for `d` in
`[ref_dist, ref_dist/min_scale]`, and only deliberately diverges outside it. But the px-per-metre
conversion used to author the values is viewport-height dependent: at 45° vertical FOV,
`px/m = window_height / (2 * d * tan(22.5°))` — 72.4 px/m at 12 m on a 720px-tall viewport. Authored
values are therefore calibrated to one resolution; taller windows and split-screen viewports change the
relative stacking.

**How to apply:** Before blaming a widget, check which path it is on, and whether the report is about
*size* (these two paths) or *spacing* (`screen_offset` vs. differing world `offset`s). When a scene
gains a `label_depth_scale` block, expect visual change to *all* world widgets in it — and re-check
`screenshot_baselines/scenes/<project>_<scene>.png` (`test_web.py`'s 4% `BASELINE_DIFF_THRESHOLD` is
loose enough that text-size-only changes can slip through). Also sanity-check `reference_distance`
against the scene's *actual* camera-to-widget distances, not just `Orbit`/`Party`
`min_radius`/`max_radius` — NPCs stand away from the orbit target, so real distance is often larger; the
default `50.0` means `(ref/dist)` never drops below 1.0 in a typical 3-18 unit zoom range and scaling
silently never engages. `min_scale` is unvalidated: `> 1.0` makes labels *grow* forever, since the
formula is `.min(1.0).max(min_floor)`.
Related: [[per-frame-changedetection-transform-writes]].

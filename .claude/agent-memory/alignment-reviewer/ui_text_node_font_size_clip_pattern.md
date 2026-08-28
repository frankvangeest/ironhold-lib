---
name: ui-text-node-font-size-clip-pattern
description: How ui-block Label/Button text sizing and Node overflow reach the designer — single shared spawn site, implicit_some authoring form, the schema's f32+default_fn vs Option<f32> convention split, and the shipped scenes that rely on text overflow bleed
metadata:
  type: project
---

Covers the screen-space `ui:` block's `Label`/`Button` (`UiNodeDef::Label`/`Button`), distinct
from the world-space `WorldLabelDef`/`EntityLabelDef` path (see [[label-depth-scale-pattern]]).

## Spawn topology — one function, two callers
`spawn_ui_element_node()` in `scene_manager/scene_loader.rs` (~1574) is the **only** site that
builds `Label`/`Button` entities. Both callers (panel mode ~1285, absolute mode ~1317) build the
`Node` **before** the call and hand it in, always with `justify_content: ui_justify(el.align())`
+ `align_items: AlignItems::Center`. So per-variant `Node` tweaks are done by mutating the
passed-in `node` inside the match arm (`btn_node.border = UiRect::all(Val::Px(5.0))` is the
pre-existing precedent). One edit covers both modes — unlike the 3-to-6-spawn-path footgun that
plagues prefab/entity markers. `UiNodeDef` variants are not matched in `ironhold_cli`, so field
adds here need no CLI touchpoint (only a `tools/bin/ironhold` cache rebuild, since
`deny_unknown_fields` makes a stale binary reject the new field).

## Designer authoring form: `Option<T>` is fine, `Some(` is not
`schema/ron_loader.rs` enables `Extensions::IMPLICIT_SOME` globally for asset RON, so a designer
writes `font_size: 14.0`, never `font_size: Some(14.0)`. The `ron_lint::no_explicit_some_in_ron_files`
test hard-fails any `Some(` under `assets/projects/`. **But** `tests/*.rs` use plain
`ron::de::from_str` with no extensions, so test fixtures must write `Some(...)` (existing
precedent: `ui_panel: Some(())` in `ron_validation.rs`). Consequence: a test-only fixture never
exercises the real designer-facing bare form — worth an extra `ron_validation` parse assertion
when a new `Option<T>` field lands.

## Schema convention split for `font_size`
The dominant convention in `schema/scene_v2.rs` is `pub font_size: f32` +
`#[serde(default = "default_x_font_size")]` (WorldLabelDef 18.0, TargetHud 16.0, Dialogue
18/15/13, Inventory 11.0, Shop 13.0, Nameplate 14.0) — the default lives in the schema where the
doc comment and any future tooling can read it. `LabelDef`/`ButtonDef` (2026-08-28) instead use
`Option<f32>` + `unwrap_or(22.0)`/`unwrap_or(26.0)` at the scene_loader call site, which splits
the default across two files. Both work; prefer the `f32` + default-fn form for new fields unless
"unset" is genuinely distinguishable from "default".

## Overflow bleed is load-bearing in shipped scenes
Adding an unconditional `overflow: Overflow::clip()` to a `Label`/`Button` `Node` is **not** a
free improvement. Real content that currently depends on bleed:
- All six `camera_modes/scenes/*.scene.ron` `hint` labels: ~100-125 chars at 22px in a
  `size: (760..900, 28)` box. Verified against `screenshot_baselines/scenes/camera_modes_flycam_test.png`:
  the text wraps to 2 lines, the 0.55-black backdrop covers only line 1, and line 2 renders
  *outside* the box. Clipping cuts line 2 in half.
- `3rd_person_game_demo` `combat_status_label`: `size: (280, 22)` with a 22px font — Bevy's
  ~1.2x line height (26.4px) already exceeds the box, so clipping shaves every glyph's
  ascender/descender. Any label whose box height <= font_size * 1.2 has this problem.
- `bind:`-driven labels (`target_display`, `score`) have runtime-variable text length, so the
  designer cannot size the box for the worst case — clipping turns a cosmetic bleed into silently
  *missing information*.
`align_items: Center` + `justify_content: Center` (the default) means clipping crops **both**
ends of an overlong string, not just the tail — reads as an engine bug, not "my box is too small".
`Overflow::clip_x()` is the safer half-measure; a RON escape hatch is safer still.

## test_web will not catch UI text regressions
`BASELINE_DIFF_THRESHOLD = 0.04` (4% of 1280x720 ~ 37k px). A clipped-away line of hint text is
~13k px. UI-only visual regressions land **silently green**. Never rely on the browser suite to
catch a UI text/layout change; inspect the affected baselines by hand
(`screenshot_baselines/scenes/*.png` are readable directly).

---
name: project-ui-vs-text2d-compositing
description: Verified — bevy_ui always composites ABOVE Text2d/Mesh2d world labels in this engine; "nameplate on top of HUD" reports are overlap/translucency, not render order
metadata:
  type: project
---

bevy_ui `Node` content always renders **above** the `Text2d`/`Mesh2d` world labels (nameplates,
`stat_label`, `world_stat_bar`) in this codebase. There is no inversion, and the commented-out
`bevy::ui::IsDefaultUiCamera` on the persistent overlay `Camera2d` (`lib.rs` `setup()`) is
genuinely redundant — do not "fix" a reported nameplate-over-HUD symptom by uncommenting it or by
re-ordering cameras.

**Why (Bevy 0.18 mechanism, checked in the registry sources):**
- `DefaultUiCamera::get()` (`bevy_ui-0.18.0/src/ui_node.rs`) falls back to
  `max_by_key(|(e, c, _)| (c.order, *e))` over cameras targeting the primary window. `RenderTarget`
  is a *required component* of `Camera` (`bevy_camera-0.18.0/src/camera.rs`), so the Camera2d can
  never drop out of that query. At `order: 1000` it always outranks every scene 3D camera
  (orders 0/1/2), so UI targets the same Camera2d that draws the world labels.
- Within one camera, `bevy_ui_render-0.18.0/src/lib.rs` wires `Node2d::EndMainPass -> UiPass ->
  Node2d::Upscaling`, so the UI pass always runs after `Transparent2d`.

**Evidence technique worth reusing:** decode `screenshot_baselines/scenes/*.png` (PIL is NOT
installed — a ~25-line zlib/PNG unfilter script works) and read exact pixels. A `Mesh2d` stat-bar
quad reading exactly 0.7x its out-of-panel color inside a 30%-black UI Label rect proves UI is on
top; antialiased *text* glyph edges are ambiguous for this and will mislead you.

**What such reports actually are:** world labels visible *through* a translucent HUD element
(`target_label`'s background is black @ ~0.30 alpha), or labels landing in the gaps between opaque
HUD widgets. That is a positioning/clutter problem — see [[project_label_depth_scale_three_mechanisms]].

---
name: ui-label-box-overflow-reliance
description: Shipped scene RON deliberately relies on Label/Button text overflowing its declared size box — any Overflow::clip() change to those nodes is a content-breaking change, not a strict improvement
metadata:
  type: project
---

`UiNodeDef::Label`/`Button` always get a **fixed pixel** `Node` box (`width: Val::Px(el.size().0)`,
never `Auto`), default `(120.0, 32.0)` via `default_ui_size`. Bevy's default font is monospace
(~0.6em advance), so at the 22px/26px defaults roughly `width / 13.2` (Label) or `width / 15.6`
(Button) characters fit on one line; longer text wraps to a second line that renders **outside**
the box — visibly, with no backdrop behind it, since `Label`'s `BackgroundColor(0,0,0,0.55)` only
covers the box itself.

**A large fraction of the shipped asset corpus depends on that overflow being visible.** Measured
2026-08-28: ~34 of 122 `Label`/`Button` defs overflow their declared width — every `camera_modes`
scene's instruction line, ~15 `local_coop_demo` rooms, `entity_logic_demo`, `custom_materials`,
`primitive_world`. Confirmed visually in `screenshot_baselines/scenes/camera_modes_flycam_test.png`:
line 1 sits on the dark backdrop, line 2 (the actual WASD instructions) hangs below it unbacked.
Three defs are also height-tight (box height < 1.2 x font size, e.g. `3rd_person_game_demo`'s
`combat_status_label` at `size: (280.0, 22.0)` with the 22px default) and would lose glyph
ascenders/descenders to a clip.

**Why:** any "add `Overflow::clip()` so text truncates instead of bleeding into neighbours" change
therefore *deletes shipped instructional text*, and does it silently — `ironhold_cli validate` and
`asset_checker.py` check references and schema, never rendered text extents. `test_web.py`'s
per-scene 4%-threshold baselines are the only automated tripwire, and the tempting response to a
wall of baseline diffs is `--update-baselines`, which bakes the loss in.

**How to apply:** treat clipping on `Label`/`Button` as opt-in per node (`clip: bool`, default
`false`), or make it opt-out, or fix the ~34 offending defs' `size:` in the same change — never
land it unconditionally on the premise that "nothing relies on overflow". Related Bevy facts worth
keeping: default `OverflowClipMargin` is `PaddingBox`, so `Button`'s hardcoded 5px border shrinks
its clip rect 10px below its authored `size:`; `clip_check_recursive` (bevy_ui `focus.rs` /
`picking_backend.rs`) only walks **ancestors**, so a node's own `Overflow` never shrinks its own
hit area — `Interaction`/hover is unaffected. Clipping costs nothing on WASM/WebGPU: it is an
`Option<Rect>` per extracted node clamped CPU-side in `bevy_ui_render`, no scissor, no extra pass,
no pipeline variant, no batch break.

See also [[ui-hover-and-tooltip]], [[demo-project-baseline-determinism]].

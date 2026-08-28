---
name: ui-label-button-font-and-clip
description: Screen-space ui: Label/Button sizing rules — the f32+serde-default house convention for every other font_size field, the 22/26px defaults, ~11px/char at 22px, and why Overflow::clip() on a wrapping centered box produces half-cut lines
metadata:
  type: project
---

About the `ui:` block's `Label`/`Button` (screen-space, `scene_loader.rs` `spawn_ui_element_node`),
as distinct from world-space `label:`/`world_labels:` (see [[world-label-legibility]]).

**`font_size` house convention is `f32` + a named serde default fn — not `Option<f32>`.**
Every one of the ~13 other `font_size`-ish fields in the schema follows it:
`EntityLabelDef`/`WorldLabelDef` (`default_wl_font_size` 18), `TargetHudDef` (16),
`StatLabelDef` (16), `WorldStatBarStyle::Ascii` (14), `DamagePopupStyle` (22),
`DialoguePanelDef` speaker/body/choice (18/15/13), `InventoryPanelDef`/`ContainerPanelDef` (11),
`ShopPanelDef` (13), `NameplateDef.name_font_size` (14). The benefit is not RON syntax —
`IMPLICIT_SOME` is enabled in `schema/ron_loader.rs`, so `font_size: 14.0` authors identically
either way and `ron_lint`'s `no_explicit_some_in_ron_files` forbids `Some(...)` regardless — it is
that the default lives beside the field with its doc comment instead of as an `unwrap_or(22.0)`
literal in `scene_loader.rs`, which is where this repo has repeatedly let defaults drift across
multiple spawn sites (see `stat_display.rs`'s six `depth_scale` sites in
[[depth-scale-field-scope]]).

**Hardcoded UI text sizes:** `Label` 22.0, `Button` 26.0. `StatBar` value text 13.0.
`StatSpread` derives its own from `row_height` (`*0.70` label, `*0.65` value, floor 10).
`size:` only ever sets the layout box + the Label's translucent backdrop, never glyph size.

**Sizing arithmetic for `ui:` text (22px):** ~11 px per average character —
900px ≈ 82 chars, 800px ≈ 73, 760px ≈ 69, 480px ≈ 43. Line height is ~1.2×font, so a 22px line
needs `size.1 >= ~27`; the common `size: (…, 26.0)` is already marginally short.
`docs/20_data_formats.md:783` states the Button equivalent (~11-12 chars per 200px at 26px).

**Clipping trap — `Overflow::clip()` + wrapping + `align_items: Center` = half-cut lines.**
The shared `Node` built at `scene_loader.rs` ~1265-1284 uses `align_items: AlignItems::Center`
(vertical) and `justify_content: h_justify` (horizontal, `Center` by default). The `Text` child
has no Node of its own, so it wraps at the parent's width and grows in height. A 2-line block
(~53px) centered in a 28px box overflows ~12px top AND bottom, so clipping does not remove
"the second line" — it shaves the top half of line 1 and the bottom half of line 2, leaving an
unreadable band. Clean truncation requires either `LineBreak::NoWrap` on the text (then clip cuts
at the right edge, which reads correctly as truncation, though with default `align: Center` it
cuts both ends) or `align_items: FlexStart` (keeps line 1 whole).

**Shipped labels that currently rely on overflowing their box** (all `camera_modes` hint labels,
22px, wrap to 2 lines): `flycam_test` 124 chars @ 900, `flycam_spectator_test` 114 @ 900,
`fixed_test` 113 @ 800, `follow_test` 97 @ 760, `first_person_test` 96 @ 760, `main` 96 @ 900 —
all `size.1 = 28`. Any "nothing relies on overflow today" claim is false; check these first.
They also all contain em-dashes (see [[em-dash-font-glyph-gap]]).

**How to apply:** when reviewing a change to `ui:` `Label`/`Button` rendering, run the char-count
arithmetic above against every shipped `Label`/`Button` before accepting a layout-behaviour change,
and remember a `feature/*` branch cut from `main` will NOT contain projects that only exist on
`integration` (`dynamic_animation_control` was the case in Aug 2026) — audit content impact against
`integration`, not the branch's own worktree.

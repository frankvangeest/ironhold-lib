# Feature: Authorable `Label`/`Button` font size + overflow clipping

_Status: Done_
_Planned at: `452e2e2` (2026-08-28)_

## What

Lets a designer set the font size of a `ui:` `Label` or `Button` directly in RON, instead of it
being permanently fixed at 22px (Label) / 26px (Button) regardless of the `size:` box authored
around it. Also adds an opt-in `clip: bool` so a box sized too small for its content can be made
to truncate instead of visually overflowing into whatever UI element sits below or beside it —
opt-in, not the default, since existing content relies on the overflow (see Approach below).

## Why

`scene_loader.rs` hardcodes `TextFont { font_size: 22.0 }` for every `Label` and `26.0` for every
`Button` — `size:` only ever sets the layout `Node`'s width/height (and, incidentally, the
translucent backdrop `Label` draws behind its own text), never the actual rendered glyph size.
Combined with no `overflow: Overflow::clip()` on either node, a designer who doesn't hand-compute
pixel widths against an unexpectedly wide proportional font (~15px/char at 22px, ~16px/char at
26px — no monospace assumption holds) gets text that silently wraps/overflows past its declared
box into whatever's positioned next, since every UI node here is absolute-positioned and nothing
auto-adjusts for a neighbor's overflow.

This has independently cost real iteration time twice: `local_coop_demo`'s room2 (2026-07-05) and
`dynamic_animation_control`'s own UI (2026-08-28) — the second occurrence is what prompted this
plan. See `planning/claude_suggestions.md` ▸ UI for both writeups and `planning/backlog.md` ▸
Queued ▸ UI for the tracked item this promotes.

## Approach

**Revised after 4 parallel post-implementation reviews (alignment-reviewer, system-architect,
debug-detective, ux-gamedesigner-reviewer) all independently converged on the same critical
finding, with measured pixel-level evidence: this plan's original premise — "nothing today relies
on a `Label`/`Button`'s text overflowing its own box on purpose" — is false.** Every `Label`/
`Button` node is built with a fixed-size `Node` (`Val::Px`, never `Auto`) and `align_items:
AlignItems::Center`; over-long text wraps and grows vertically, spilling symmetrically above and
below the box, fully legible. `system-architect` and `debug-detective` both measured this directly
against `screenshot_baselines/scenes/camera_modes_flycam_test.png`: an unconditional
`Overflow::clip()` doesn't truncate that spill, it slices the top off line 1 and the bottom off
line 2 — a *worse* failure mode (illegible chopped glyphs) than today's overflow, hitting ~34
`Label`/`Button` defs across six shipped projects (`camera_modes` all 6 scenes, several
`local_coop_demo` rooms, `entity_logic_demo`, `custom_materials`, and a `bind:`-driven `flycam_
position` label whose runtime text length isn't statically knowable at all). `debug-detective` also
confirmed `test_web.py`'s 4%-of-frame baseline diff threshold cannot catch this — the chopped text
is well under 1% of a frame, so it would land silently green.

### Schema — REVISED: `f32` + default fn, not `Option<f32>`

Per both `alignment-reviewer` and `ux-gamedesigner-reviewer`: every other font-size field in this
schema (`EntityLabelDef`/`WorldLabelDef`, `TargetHudDef`, `StatLabelDef`, `DialoguePanelDef`,
`InventoryPanelDef`/`ShopPanelDef`/`ContainerPanelDef`, `NameplateOptionsDef`) is `f32` +
`#[serde(default = "default_x_font_size")]`, not `Option<f32>`. There's no case here where "unset"
needs to be distinguishable from "set to the default", so matching house convention:

```rust
#[serde(default = "default_label_font_size")]  // / "default_button_font_size"
pub font_size: f32,
```

### Clipping — REVISED: opt-in `clip: bool`, default `false`, paired with top-anchoring

Rather than unconditional clipping, `clip: bool` (`#[serde(default)]`, i.e. `false`) on both
`LabelDef`/`ButtonDef` — `system-architect`'s and `debug-detective`'s preferred fix among the
options they each raised. This means **zero existing RON needs to change**: every one of the ~34
overflowing defs keeps rendering exactly as it does today, unaffected. Only a designer who
explicitly opts in gets clipping — e.g. a `bind:`-driven label whose runtime text length varies
and must never spill into a neighboring element.

When `clip: true`, the node ALSO switches `align_items` from `Center` to `FlexStart` (top-anchor)
— this is what makes opted-in clipping mean genuine truncation (line 1 fully legible, only
trailing/bottom content that doesn't fit is cut) instead of reproducing the "chop every line in
half" problem the reviews found, which is specific to clipping *centered* content.

### `font_size <= 0.0` validation — added per `debug-detective`'s finding

Confirmed via vendored Bevy source: `font_size <= 0.0` doesn't panic, it silently renders nothing,
and the one `warn!` Bevy itself logs fires via `once!` — a per-process flag, so only the very
first offending entity in the whole session is ever reported; a second mistake anywhere, or the
same one on a scene reload, produces zero diagnostic. `ironhold_cli validate` now rejects
`font_size <= 0.0` on any `Label`/`Button` as `invalid_font_size`, catching this at design time
instead.

### Worked example

`3rd_person_game_demo`'s `toggle_nameplate_button` (`scenes/main.scene.ron`) previously had to
shorten its label from `"Toggle Nameplate"` to `"Nameplate"` to fit the old fixed-26px font in a
200px-wide button — restored to the full text with `font_size: 20.0` (fits 16 chars in 200px per
the sizing note below), removing the stale workaround comment. This is also the project's first
shipped usage of the new field, per `ux-gamedesigner-reviewer`'s "no worked example anywhere"
finding.

### Scope boundary

`IconButton` and other non-text UI node types are untouched — this plan is scoped to the two node
types that actually render designer-authored text at a fixed size today. `EntityLabelDef`/
`WorldLabelDef` (the world-space label mechanism, distinct code path in `scene_loader.rs`/`lib.rs`)
already have an authorable `font_size` field — this plan only closes the gap on the screen-space
`ui:` block's `Label`/`Button`.

No `Action`/schema-version bump needed — this only touches `UiNodeDef`'s two variants, not the
`Action` enum, so no `ironhold_cli` `query actions`/validate cross-file work is required beyond a
compile-check.

## Tasks
- [x] Add `font_size: f32` (+ `default_label_font_size`/`default_button_font_size`) to `LabelDef`/`ButtonDef` (`schema/scene_v2.rs`)
- [x] Add `clip: bool` (`#[serde(default)]`) to `LabelDef`/`ButtonDef`
- [x] `scene_loader.rs`: use `label.font_size`/`btn.font_size` directly (no more `unwrap_or`)
- [x] `scene_loader.rs`: gate `overflow: Overflow::clip()` + `align_items: AlignItems::FlexStart` behind `label.clip`/`btn.clip`
- [x] `ironhold_cli validate`: reject `font_size <= 0.0` on any `Label`/`Button` (`invalid_font_size`)
- [x] Tests — `test_label_and_button_font_size_override`/`_default_unchanged` (font size), plus
      `test_label_and_button_clip_defaults_to_off_and_preserves_center_alignment`/
      `_clip_true_enables_clipping_and_top_anchors` (the clip/align_items behavior — the riskier
      half of the change, flagged as untested in the first review pass)
- [x] Docs — `docs/20_data_formats.md`'s `Label`/`Button` field tables + rewritten sizing note
      (was actively false: claimed the field didn't exist)
- [x] Worked example — `3rd_person_game_demo`'s `toggle_nameplate_button` restored to its full text
- [x] Full `ironhold_cli validate` sweep + `ron_lint`/`ron_validation` across all affected projects
- [x] Full `cargo test -p ironhold_core --test '*'` + `cargo check -p ironhold_cli` green
- [x] WASM dev build + playtest — specifically confirm `toggle_nameplate_button` renders correctly
      and that no other shipped `Label`/`Button` visually changed (should be none, since `clip`
      defaults off and no other RON was touched)

## Open questions
- Should `clip` apply to `IconButton` too? No — `system-architect` found a concrete reason beyond
  scope: `IconButton`'s drop-shadow child is deliberately `PositionType::Absolute` offset from the
  parent box, so clipping the parent would eat the shadow. Stays out of scope permanently, not
  just deferred.
- The other hardcoded font sizes in composite widgets (action-bar value text, inventory/shop/
  container panel headers, `camera.rs`'s split-screen "P1"/"P2" label) are the same class of
  problem — logged to `planning/backlog.md` as a separate follow-up per `system-architect`'s
  recommendation, not folded into this change.
- An `Auto`-sized `Label`/`Button` box (size-to-content, no manual pixel budgeting at all) would
  be a better long-term answer than either `font_size` or `clip` — logged to
  `planning/claude_suggestions.md` per `system-architect`'s suggestion, not attempted here.

## Acceptance criteria
- Given `Label(font_size: 14.0, ...)`, when the scene loads, then that label's rendered text is
  14px, not the hardcoded 22px default.
- Given a `Label`/`Button` with no `font_size` set, when the scene loads, then it renders at
  exactly the same size as before this feature (22px / 26px) — no default-value regression.
- Given a `Label`/`Button` with no `clip` set, when the scene loads, then overflowing text spills
  past the box exactly as it did before this feature — no default-behavior regression across any
  of the ~34 existing defs that currently rely on this.
- Given `clip: true` on a `Label`/`Button` whose text overflows its box, when the scene loads,
  then the text is top-anchored and clipped at the box edge — line 1 fully legible, only
  trailing/bottom overflow is cut, never a jagged slice through every line.
- Given `font_size: 0.0` (or negative) on a `Label`/`Button`, when validated, then
  `ironhold_cli validate` reports `invalid_font_size` rather than shipping a silently-invisible
  label.

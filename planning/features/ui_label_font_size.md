# Feature: Authorable `Label`/`Button` font size + overflow clipping

_Status: Draft_
_Planned at: `452e2e2` (2026-08-28)_

## What

Lets a designer set the font size of a `ui:` `Label` or `Button` directly in RON, instead of it
being permanently fixed at 22px (Label) / 26px (Button) regardless of the `size:` box authored
around it. Also clips text to its `Node` box by default, so a box sized too small for its content
truncates instead of visually overflowing into whatever UI element sits below or beside it.

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

### Schema

Add one optional field to each of `LabelDef` and `ButtonDef` (`schema/scene_v2.rs`):

```rust
#[serde(default)]
pub font_size: Option<f32>,
```

`None` (the default — every existing scene's RON is unaffected) falls back to the current
hardcoded value at the one call site each type has in `scene_loader.rs`:
`label.font_size.unwrap_or(22.0)` / `btn.font_size.unwrap_or(26.0)`. This is purely additive —
zero behavior change for any RON that doesn't set the new field.

### Clipping

Add `overflow: Overflow::clip()` to the `Node` both `Label` and `Button` spawn with, unconditionally
— not an opt-in field. Nothing today relies on a `Label`/`Button`'s text overflowing its own box
on purpose (that was always the bug this whole plan exists to close), so this is a strict
improvement with no opt-out needed: a still-too-small box now truncates instead of bleeding into a
neighboring element, which is a better failure mode in every case.

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
- [ ] Add `font_size: Option<f32>` to `LabelDef` (`schema/scene_v2.rs`)
- [ ] Add `font_size: Option<f32>` to `ButtonDef` (`schema/scene_v2.rs`)
- [ ] `scene_loader.rs`: use `label.font_size.unwrap_or(22.0)` at the `Label` spawn site
- [ ] `scene_loader.rs`: use `btn.font_size.unwrap_or(26.0)` at the `Button` spawn site
- [ ] `scene_loader.rs`: add `overflow: Overflow::clip()` to both nodes' `Node` component
- [ ] Tests — a scene-load / RON-parse test confirming `font_size: Some(n)` overrides the
      rendered `TextFont.font_size`, and confirming omitting it still reproduces the current
      22.0/26.0 defaults exactly (regression guard against ever silently changing the default)
- [ ] Docs — `docs/20_data_formats.md`'s `Label`/`Button` field tables (add the new field row);
      mention the ~15px/22px, ~16px/26px empirical character-width figures somewhere designers
      will actually see them before hand-computing a box width again

## Open questions
- Should the clip default apply to `IconButton` too, or any other UI node with a `Node`? Deferred
  — scoped to `Label`/`Button` only, since those are the two types this feature's motivating
  incidents actually hit. Revisit if a future incident hits an `IconButton`/other node type.
- Is a minimum/maximum sane `font_size` worth an `ironhold_cli validate` check (e.g. reject
  `<= 0.0` or absurdly large values)? Leaning no for v1 — `TextFont` presumably already handles a
  degenerate value gracefully (renders nothing/tiny), and this isn't the kind of mistake that
  silently produces a "looks fine but is subtly wrong" result the way an unvalidated action
  reference does. Revisit if it turns out not to fail gracefully.

## Acceptance criteria
- Given `Label(font_size: 14.0, ...)`, when the scene loads, then that label's rendered text is
  14px, not the hardcoded 22px default.
- Given a `Label`/`Button` with no `font_size` set, when the scene loads, then it renders at
  exactly the same size as before this feature (22px / 26px) — no default-value regression.
- Given a `Label`/`Button` whose text is too long for its `size:` box, when the scene loads, then
  the text clips at the box edge instead of visibly overflowing into a neighboring UI element.

---
name: actionslotdef-label-undocumented
description: ActionSlotDef.label is NOT rendered anywhere (future-use tooltip); key_hint is the only on-screen slot text; docs table now lists both but omits the not-yet-rendered caveat
metadata:
  type: project
---

`ActionSlotDef` has two text-ish fields, and only ONE of them renders:

- **`key_hint: Option<String>`** (added by the action-bar-custom-hotkeys feature) — renders as the
  bottom-right **corner glyph** of the slot. When omitted, the corner shows a pretty-print of
  `key` (strips only the `"Key"` prefix, so `"KeyQ"` -> `"Q"`; digits and `"F2"` render as-is;
  `"ShiftLeft"`/`"ArrowUp"` render literally — recommend `key_hint` for those).
- **`label: Option<String>`** — **NOT rendered anywhere.** Genuinely "future use" (a hover tooltip
  that does not exist yet). Verified: neither `scene_loader.rs`'s ActionBar arm nor
  `capabilities/action_bar.rs` reads `slot.label`. Pre-feature the bar rendered only
  `Text::new(key.clone())`.

**CORRECTION of a prior wrong memory:** an earlier version of this note (and the feature plan's own
analysis) claimed the bar "renders TWO distinct texts per slot (corner key-hint AND label)." That
was FALSE — `label` has never rendered on the bar. Only the corner hint renders.

**Doc state after the feature:** `docs/20_data_formats.md` ActionSlotDef field table now DOES list
both `label` and `key_hint` (the pre-existing gap I flagged is fixed). BUT the `label` row reads
"Tooltip/ability name (e.g. `"Heavy Strike"`)" with NO "(not yet rendered / future use)" caveat —
even though the schema doc comment in `scene_v2.rs` does say "(future use)". So a designer who sets
`label: "Battle Cry"` (as the docs example AND the shipped `3rd_person_game_demo` KeyE slot both do)
sees it nowhere. This directly undercuts the demo slot's stated purpose as a copy-reference for the
label-vs-key_hint distinction: the distinction is half-invisible.

**How to apply:** when reviewing action-bar docs/examples, insist the `label` doc row carries an
explicit "not yet displayed — reserved for a future hover tooltip" note, matching the schema comment.
Any example/demo that sets `label` should either drop it or annotate that it currently produces no
visible output.

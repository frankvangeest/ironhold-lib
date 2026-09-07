---
name: embedded-font-is-ascii-only
description: Bevy's embedded default font maps exactly 95 codepoints (printable ASCII only) — EVERY non-ASCII char in any engine text tofus, not just em-dashes
metadata:
  type: project
---

Every text node in ironhold uses `TextFont { ..default() }` (grep: ~20 sites in
`scene_loader.rs`), i.e. Bevy's embedded `FiraMono-subset.ttf`. I enumerated that font's `cmap`
directly (at `~/.cargo/registry/src/*/bevy_text-0.18.0/src/FiraMono-subset.ttf`, format-4 subtable
walk in Python): it maps **exactly 95 codepoints — U+0020..U+007E and nothing else**. Zero
non-ASCII glyphs.

**Why:** the "em-dash renders as a tofu box" incident in `planning/claude_suggestions.md` is not
about dashes at all — it's the whole non-ASCII range. Any lint, doc, or fix framed as "non-ASCII
*dash*" is under-inclusive by construction. Curly quotes (U+2019 — the likeliest paste-in char),
`…`, `·`, `→`, `×`, `°`, U+00A0 NBSP (invisible char becoming a visible box), and all accented
Latin (é/ñ/ü — so any non-English UI text is solid tofu) fail identically with no runtime warning.

**How to apply:** when a text-rendering bug is reported as "wrong character"/"box"/"missing
glyph", the answer is almost always "that codepoint is > 0x7E". The correct invariant for any
validator is `!c.is_ascii()` (plus control chars other than `\n`), never a hand-listed character
set. `assets/projects/particles_demo` and `effect_mayhem_demo` shipped ~20 such `text:`/
`world_labels:` strings as of 2026-09-07. Re-derive the cmap rather than trusting this count if
the bevy version changes. Related: [[dynamic_animation_control UI legibility]] (font size is also
hardcoded at those same sites).

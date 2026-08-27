---
name: em-dash-font-glyph-gap
description: The engine's embedded UI font has no glyph for em-dash (—) — renders as a tofu box in any in-game text; always flag it, always recommend ASCII hyphen
metadata:
  type: project
---

The game's embedded UI font has **no glyph for U+2014 (em-dash, `—`)** — it renders as a visible
tofu box wherever it appears in any `text:` field that goes through the engine's own text
rendering (scene `ui:` `Label`/`Button`, entity `label:`, `world_labels:`, dialogue `body:`,
`ShowFloatingText`/`ShowDamagePopup` text). Confirmed via byte-level comparison
(`\xe2\x80\x94` — the same UTF-8 sequence in both a "looks fine at a glance" and a visibly-broken
screenshot), so this is a font coverage gap, not an encoding mistake, and it is **not** limited to
one project — `custom_materials/scenes/main.scene.ron`'s own shipped baseline screenshot showed it
too, discovered while auditing `dynamic_animation_control`'s UI (2026-08-28).

**When reviewing any RON authoring a designer-facing `text:`/`body:` string, check for `—` and
flag it every time** — recommend a plain ASCII hyphen `-` instead. This applies to new content in
any project, not just ones already known to have the issue. Does NOT apply to `.md` docs or RON
comments (`//`) — those render in a markdown viewer/IDE, not through this engine's font, so
em-dash is fine there.

Tracked in `planning/backlog.md` ▸ Bugs (still open — the underlying font gap itself isn't fixed,
only worked around via content). `docs/20_data_formats.md`'s Label depth scaling section now has a
callout stating this rule for designers. See also [[project_world_label_legibility]] for the
broader label-authoring context this was found alongside.

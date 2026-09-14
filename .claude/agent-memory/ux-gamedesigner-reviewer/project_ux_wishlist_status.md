---
name: ux-wishlist-status
description: My top-5 designer-experience wishlist in planning/stakeholder_priority_list.md and which items are still open as of 2026-09-14
metadata:
  type: project
---

I own the "UX-Gamedesigner-Reviewer" section of `planning/stakeholder_priority_list.md`
(snapshot `db1ede0`, 2026-09-03). Status re-checked 2026-09-14:

- #1 RON typos silently no-op — SHIPPED (`action_deny_unknown_fields`; also fixed `tracing-wasm`
  never binding `console.error`/`console.warn`, so browser errors are now actually visible).
- #2 `cli validate` inconsistency — heavily closed; ~12 consecutive branches since the snapshot
  each added cross-file checks. Remaining gaps are logged in `planning/claude_suggestions.md`
  ▸ CLI / Tooling, not undiscovered.
- #3 Demo projects (`prefab_demo`, `ui_demo`, `audio_demo`, `scene_transitions_demo`,
  `parkour_demo`) — ALL still unbuilt, backlog ▸ Queued ▸ Designer Experience.
- #4 RON parse footguns — only the `camera_mode` double-paren gotcha is documented
  (`docs/20_data_formats.md` ~2418); there is still no general RON-syntax primer section and no
  rule for quoted-string vs bare-enum fields.
- #5 Em-dash/tofu — lint only (`non_ascii_char_in_text`, `--strict`); the 20 pre-existing tofu
  strings in `particles_demo`/`effect_mayhem_demo` are unfixed and `FiraMono-subset.ttf` still
  covers only `U+0020..U+007E`.

**Why:** Frank periodically asks each reviewer persona to self-assess progress against this list.
**How to apply:** When asked "what should be pulled forward," lead with #3 (demo projects) — it is
the only item where nothing at all has moved and it is pure designer-facing authoring work.
Related: [[project_em_dash_font_glyph_gap]], [[project_ron_enum_double_paren]],
[[project_quoted_string_vs_enum_house_style]], [[project_validate_coverage_gaps]].

---
name: ux-wishlist-status
description: My top-5 designer-experience wishlist in planning/stakeholder_priority_list.md — old list's status and the refreshed 2026-10-01 top-5
metadata:
  type: project
---

I own the "UX-Gamedesigner-Reviewer" section of `planning/stakeholder_priority_list.md`
(old snapshot `db1ede0`, 2026-09-03). Refreshed 2026-10-01:

Old list status:
- #1 RON typos silently no-op — SHIPPED (`3677859`, Action + 8 FSM/dialogue containers).
- #2 validate inconsistency — mostly closed (~10 Done CLI-validate entries in Sept). Residuals:
  stale `logic/rules.ron` unflagged, ContainerPanel slots vs max_slots, `{target}` gating.
- #3 demo projects (prefab/ui/audio/scene_transitions/parkour) — STILL all unbuilt.
- #4 RON parse footguns — still only the camera_mode gotcha (docs/20 ~2416); no general primer.
- #5 tofu — lint shipped; the 20 shipped strings (particles_demo 16, effect_mayhem_demo 4) and
  the ASCII-only font are unchanged.

Refreshed top-5 (2026-10-01): 1 real pause (docs teach the cosmetic pattern), 2 load failures
hang with no on-screen error (.project.ron + scene fetch), 3 demo projects (carried),
4 `{target}` silently -> "" for 8 action types, 5 `motion:` undocumented (used in 5 projects).

**Why:** Frank periodically asks each reviewer persona to self-assess against this list.
**How to apply:** next refresh, re-verify these five first; demo projects have now gone two
cycles with zero movement — call that out explicitly.
Related: [[pause-is-cosmetic]], [[ron-parse-failure-diagnostics]], [[em-dash-font-glyph-gap]],
[[validate-coverage-gaps]].

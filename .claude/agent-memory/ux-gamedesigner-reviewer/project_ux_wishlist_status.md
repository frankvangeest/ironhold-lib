---
name: ux-wishlist-status
description: My top-5 designer-experience wishlist in planning/stakeholder_priority_list.md — history across editions and the 3rd-edition (2026-10-13) top-5
metadata:
  type: project
---

I own the "UX-Gamedesigner-Reviewer" section of `planning/stakeholder_priority_list.md`.
Editions: 1st `db1ede0` (2026-09-03), 2nd `ce77bdd` (2026-10-01), 3rd at `929b7d1` (2026-10-13).

2nd-edition items, status at 3rd edition (none shipped — October went to UI layout/responsive work):
- #1 real pause — still open; docs/30 ~223-257 pause example still has no caveat; new Bug
  "dying while paused never reaches game over" (primitive_world state_machine:130).
- #2 load hang — still open; project_loader Failed arms are catalogs only; no
  UntypedAssetLoadFailedEvent; play.html message (b5c1800) covers only engine-can't-start.
- #3 demos — Changed: minimal `ui_demo` shipped (1167eb9, +SetUiVisible d50045d);
  prefab/audio/scene_transitions/parkour still unbuilt (3 cycles).
- #4 `{target}` -> "" — still open, action_needs_target still 11 arms (action_bar.rs ~429).
- #5 `motion:` undocumented — still open (14 uses / 5 projects, 0 docs hits).
- Tofu: still exactly 20 strings (particles_demo 16, effect_mayhem 4); re-flagged backlog ~260.

3rd-edition top-5: 1 real pause (carried), 2 action-bar slots are validate's blind spot
(collect_actions skips slot do_actions + {target}), 3 load hang (carried), 4 docs/20 too big
(354k chars) + motion: hole, 5 demo projects (prefab/audio/scene_transitions) + README table
missing particles_demo/local_coop_demo rows.

**Why:** Frank periodically asks each reviewer persona to self-assess against this list.
**How to apply:** next refresh, re-verify these five first; pause and load-hang have now gone two
editions with zero movement — call that out.
Related: [[pause-is-cosmetic]], [[ron-parse-failure-diagnostics]], [[em-dash-font-glyph-gap]],
[[validate-coverage-gaps]], [[ui-trigger-wiring]].

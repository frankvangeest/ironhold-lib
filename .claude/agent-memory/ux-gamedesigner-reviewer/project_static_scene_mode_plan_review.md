---
name: static-scene-mode-plan-review
description: static_scene_mode (?static=1 harness freeze) plan + impl review 2026-10-23 - game.paused tracks Menu hold only; docs/30 ~531 summary bullet left stale; correctly kept out of designer docs
metadata:
  type: project
---

Plan-review at `e2a5578` (2026-10-23): Needs more design work (small). Re-planned and implemented the same day.

Post-implementation docs review (2026-10-23):
- Implemented as recommended: `PauseHold::announces` (Static does not announce), static only on `test_screenshot_scene_baseline`, `STATIC_EXCLUDED_PROJECTS` = particles_demo, effect_mayhem_demo, dynamic_animation_control. `run_mode` in debug JSON is asserted.
- `?static=1` correctly stays out of designer docs (needs `testing=1`). Do not add it to docs/20, docs/30 or STATUS.md.
- Wording updated in docs/30 ~123/~125/~281 and docs/20 ~4020, but the docs/30 actions summary bullet (~531) still said "only when the clock really stops". That is the general lesson: the PauseGame contract is written in 4+ places (docs/30 event list, pause table, actions summary; docs/20 action table; STATUS.md ~89). Grep all of them on any pause-contract change.
- browser_tests.md "Static baselines" does not say that `ResumeGame` under a Static hold emits `game.resumed` but the clock stays frozen. Exclusion is per project only; per-scene opt-out is not supported.

**How to apply:** for future pause or clock-hold changes (FocusLoss, Session holds), check the announce rule and all 5 doc surfaces. Related: [[real-pause-plan-review]], [[particle-warmup-recipe]], [[dynamic-animation-control-demo]].

---
name: fixed-tick-timers-phase1-review
description: fixed-tick Phase 1 (timers on 1/64 s step) docs+validate review 2026-11-19; open traps: 0.0156 advice < 1/64, re-arm "no drift" claim, interim callout cites planning/
metadata:
  type: project
---

Reviewed `feature/fixed_tick_timers` (2026-11-19). Delayed events, cooldowns, stat regen and modifier
durations now count 1/64 s steps (FixedUpdate, before Update's targeting/action bar/interact/backend).

Open items found (check whether fixed before re-reviewing later phases):
- `sub_step_duration` validate advice says "at least 0.0156", but 0.0156 < 1/64 (0.015625), so following
  the advice re-triggers the warning. Recommend "0.016".
- docs/20 "Timing resolution" claims a self re-arming timer "does not drift with the frame rate". Wrong
  in the strict sense: if it fires on a non-last step of a multi-step frame, the re-arm starts after the
  remaining step(s), so the period is N or N+1 steps (slow machines drift up to 1 step per cycle).
- docs/30 "Changed in this release (interim)" callout points designers at `planning/features/...`
  and says "interpreters" (see [[ron-comments-cite-dev-paths]]).
- The Timing resolution section sits at the end of docs/20 with no link from the `cooldown_secs`,
  `regen_rate`/`regen_delay`, modifier duration or `EmitEventAfterDelay` rows.

**Why:** later phases (2-5) rewrite the same docs/30 frame-order list and the same section; these are the
spots that drift.
**How to apply:** on Phase 3/4 reviews, re-check the frame-order list in docs/30 "Same-frame event order",
the interim callout (must be removed in Phase 4), and the Timing resolution bullets (despawn/dialogue move
in Phase 4). Related: [[same-frame-event-order]], [[fixed-tick-pipeline-review]].

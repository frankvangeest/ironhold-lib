---
name: fixed-tick-pipeline-review
description: Designer-facing facts from the 2026-10-02 and 2026-11-19 plan-reviews of gameplay_fixed_tick_pipeline.md - timer resolution, pause contract already shipped, per-phase doc surfaces that flip
metadata:
  type: project
---

Plan `planning/features/gameplay_fixed_tick_pipeline.md` moves timers (Phase 1) then interpreter/executor
(Phase 4) to 64 Hz FixedUpdate. Authored values stay seconds; timers fire on first tick >= deadline.

- Shortest shipped timers: 0.05 s / 0.18 s flicker loops; 0.2/0.3 s cooldowns. None sub-tick-sensitive.
- 3rd_person monster respawn is 30 s (enemy_*.behavior.ron).
- **Pause contract is ALREADY SHIPPED (real_pause, 2026-10-14):** docs/30 ~317-321 freeze/keep-running
  list, docs/30 ~343 + docs/20 EmitEventAfterDelay row, validate `delayed_event_while_paused` (docs/15
  ~225, --strict only). Rev-2 plan re-proposed both as new (Q4) and its table was LESS complete than docs/30
  (omitted bag-slot clicks frozen docs/20 ~5143, slot clicks ignored docs/20 ~1182, interact/target inputs).
  At implementation review: check bag-slot / action-bar / world clicks while paused stay no-ops after the
  capture move (Phase 3/4).
- No shipped demo arms a delay/despawn timer from a pause state. Camera-shake-while-paused test: local_coop_demo
  `shake_camera` key (global_on). first_person_starter opens its menu from `game.paused` (flat pattern).
- Old docs/30 "System ordering" block (~873-884) no longer exists (D3 rewrite); live surfaces now:
  docs/30 ~207, ~321, ~384-395, ~406-407, ~426, ~464-467, ~686, ~49; docs/20 ~1172, ~4913, ~5446,
  ~4049; STATUS ~94, ~124 (DebugState fields); spawn_wave_encounter.md ~283.
- Phase 1 alone flips docs/30 ~387-389 (delayed/modifier move ahead of targeting/bar/interact), ~426
  (regen same-frame), ~464 (backend reply order). Rev 2 had NO Phase 3 docs bullet despite its table claiming one.
- Spawn cap decision: 2/tick logical (=128/s) + per-frame render cap => visible appearance still
  frame-rate dependent; docs must not say "N per second on every machine" for appearance.
- SimTick in DebugState is scheduled Phase 5 but the playtest checklist uses it from Phase 1.

**How to apply:** at each phase's implementation review, grep these surfaces. Related:
[[same-frame-event-order]], [[real-pause-plan-review]], [[max-fixed-delta-secs]].

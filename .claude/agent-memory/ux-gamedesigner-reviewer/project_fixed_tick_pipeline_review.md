---
name: fixed-tick-pipeline-review
description: Designer-facing facts from the 2026-10-02 plan-review of gameplay_fixed_tick_pipeline.md - timer resolution, shortest shipped delays, pause-freezes-menu-delays footgun, "per frame" doc surfaces to flip
metadata:
  type: project
---

Plan `planning/features/gameplay_fixed_tick_pipeline.md` moves timers + interpreter/executor to 64 Hz
FixedUpdate. Authored values stay seconds; timers fire on first tick >= deadline (<=15.6 ms late).

- Shortest shipped timers: 0.05 s / 0.18 s flicker loops (particles_demo, effect_mayhem_demo torch,
  star_shower, lightning_orb); 0.2/0.3 s cooldowns in 3rd_person main.scene.ron. None sub-tick-sensitive.
- 3rd_person monster respawn is 30 s (enemy_*.behavior.ron), corpse 600 s / 20 s - plan said "60 s".
- **Pause footgun:** with real pause (Virtual freeze + "paused control tick"), anything time-based armed
  FROM the pause menu (EmitEventAfterDelay toast, delayed LoadScene, despawn fade, dialogue auto-advance,
  action-bar cooldowns from non-state-gated slots) freezes until resume. Ask for a "runs/frozen while
  paused" table in real_pause + docs/30.
- "per frame" doc surfaces that flip: docs/30 ~49/~186/~203/~310/~335/~390/~873-884, STATUS ~91
  ("max 2/frame" spawn), docs/20 ~4628 (max_fixed_delta_secs note), docs/50 ~54, docs/10 ~37,
  docs/40 ~104/~216-223, spawn_wave_encounter.md ~283.
- Spawn cap goes 2/frame -> 1/tick = 64/s on every machine; express as a rate in docs.

**How to apply:** at Phase 1/3/4 implementation reviews, grep these surfaces; insist on SimTick in
DebugState so "same tick" playtest claims are observable. Related: [[same-frame-event-order]],
[[pause-is-cosmetic]], [[max-fixed-delta-secs]].

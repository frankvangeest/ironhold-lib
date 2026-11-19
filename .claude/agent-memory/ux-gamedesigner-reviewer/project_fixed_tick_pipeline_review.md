---
name: fixed-tick-pipeline-review
description: Designer-facing facts from the 2026-10-02 and 2026-11-19 (rounds 2+3) plan-reviews of gameplay_fixed_tick_pipeline.md - timer resolution, pause contract, per-phase doc surfaces, round-3 blockers
metadata:
  type: project
---

Plan `planning/features/gameplay_fixed_tick_pipeline.md` moves timers (Phase 1) then interpreter/executor
(Phase 4) to 64 Hz FixedUpdate. Authored values stay seconds; timers fire on first tick >= deadline.

- Shortest shipped timers: 0.05 s are one-shot ENTRY delays (torch/star_shower/lightning_orb); the loops are
  0.18-0.30 s. Cooldowns 0.2/0.25/0.3. None sub-tick-sensitive. 3rd_person monster respawn 30 s.
- Pause contract SHIPPED (real_pause): docs/30 317-321, 343-345; docs/20 4049, 5143 (bag clicks), 1182 (bar
  clicks), 1467 (PauseGame on panel open - tension with 5143). Rev 3 now preserves it (round-2 B1 closed).
- Round 3 (2026-11-19) blockers I raised:
  1. Phase 3 puts the BackendReply applier in FixedUpdate before the control tick exists (Phase 4) => replies
     stall while paused, contradicting docs/30 459-460 and docs/20 5446 ("arrive while paused").
  2. Phase 3 scene-load-frame change (gate or accept) flips docs/30 392-395; Phase 3 docs bullet omits it.
  3. Phase 4 tick order puts GameStreamSet::World (pickups/trigger/NPC) AFTER Targeting/ActionBar/Interact;
     shipped docs/30 386-387 has physics events BEFORE them (D3 has no World set; physics is FixedUpdate first).
  4. Playtest method relies on a "diagnostics HUD" (F3) that is only PLANNED (docs/dev/70 marks it 🧭, no
     `diagnostics` cargo feature) and on comparing SimTick stamps no designer can see per event.
- Doc lines the plan misses: DebugState field lists docs/30 59/603/688 + STATUS 124/127 go stale at Phase 1
  (SimTick added then, plan defers to Phase 5); docs/20 4420 (threshold next frame), 5224 (item slot follows
  bag same frame), 3365-3370 (join pad valid one frame), 1164/5227 "same frame" wording - all Phase 4.
- "1/64 s per hop" is SLOWER than today on 144 Hz screens (6.9 ms/frame hop) - plan frames it as a gain.
- Spawn cap 2/tick: whole drain in tick, so 128/s is honest; 30 fps = 4/frame, 250 ms burst = 32 at default.

**How to apply:** at each phase's implementation review, grep these surfaces. Related:
[[same-frame-event-order]], [[real-pause-plan-review]], [[max-fixed-delta-secs]].

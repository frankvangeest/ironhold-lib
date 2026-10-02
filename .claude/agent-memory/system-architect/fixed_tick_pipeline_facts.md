---
name: fixed_tick_pipeline_facts
description: Verified Bevy 0.18 facts + recurring traps for moving the gameplay pipeline into FixedUpdate (gameplay_fixed_tick_pipeline.md review 2026-10-02)
metadata:
  type: project
---

Verified against vendored bevy_time/bevy_ecs/bevy_app 0.18.0 during plan-review of `planning/features/gameplay_fixed_tick_pipeline.md` (verdict: needs more design work, 5 blocking).

Bevy facts:
- `run_fixed_main_schedule` swaps generic `Time` to `Time<Fixed>` per tick, restores Virtual after (fixed.rs:240-256).
- Message buffers flip only when `MessageRegistry.should_update == Ready`, set by `signal_message_update_system` in FixedPostUpdate → max one flip per frame, none on 0-tick frames. Cross-schedule delivery exactly-once; a *gated* reader (run_if false) across 2 flips loses messages.
- Every schedule run applies final deferred commands (`apply_final_deferred` default true) → FixedUpdate commands visible to Update same frame.
- `TimeUpdateStrategy::ManualDuration`: FIRST `app.update()` runs 0 ticks (Real::update_with_instant returns early on first call). `FixedTimesteps(n)` strategy exists for N-tick tests.
- `accumulate_overstep(period)` while Virtual paused → one tick per frame with dt=1/64 AND advances Time<Fixed>::elapsed. `world.run_schedule(FixedMain)` from an exclusive system instead keeps generic Time = Virtual (dt 0).
- `.before/.after(fn)` against a system absent from that schedule is silently a no-op → moving a system across schedules kills edges without error.

Recurring traps found:
- D3's `Execute` chain includes cosmetic particle systems (simulate_pool, rebuild_pool_meshes, decals) — never re-home it wholesale.
- "Pause control tick" must whitelist control sets (Interpret/Execute), not blacklist sim sets; cooldown_tick/despawn_timer/dialogue timer are dt-consumers hidden in "control-looking" sets.
- A SimInput latch cleared at end of each tick is invisible to Update readers.
- `audio_state_system.before(fsm_interpreter_system)` exists for mute_on_start ordering — easy to forget.

**How to apply:** when reviewing any schedule-move plan, check these before trusting cross-schedule claims. Related: [[schedule_update_vs_fixedupdate]], [[schedule_ordering_mechanism]], [[time_and_animation_advance]].

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

**Round-2 review (2026-11-19, plan rev 2 at ef169e7; verdict: needs more design work):**
- Control-tick trigger must NOT be `!SimClock::is_running()` alone: `spawn_scene_v2` (Update) releases `Menu` while `Time<Virtual>` is still paused; with `apply_sim_clock` in FixedUpdate, zero ticks run and nothing ever unpauses (deadlock). Trigger on `Time<Virtual>::is_paused() || !running`; latch `SimActive = running && !virtual_paused`.
- Rapier `TimestepMode::Fixed` steps its own `dt` (bevy_rapier3d-0.33 context/mod.rs:809), ignoring `Time` → "dt=0 second line of defence" is false for physics; ticks k+1..N after a mid-frame pause also run with generic Time = Time<Fixed> (dt 1/64).
- bevy_ecs 0.18 `Dag::flatten` (graph/dag.rs ~585): an EMPTY set that has configured neighbours passes edges through (a->S->b becomes a->b); an unconfigured set in that schedule is a silent no-op. A dead-edge test must check set targets too, not only system-type targets.
- Hidden capture->executor channels beyond LoadedInventoryUi: `PendingJoinGamepad` (Entity, reset to None at the top of every `unclaimed_gamepad_trigger_system` run, input.rs:105, taken by JoinPlayer action_executor.rs:1669); `menu_action` is a single `Option` (overwritten across a 0-tick frame); `backend_glue_system` writes `GameVariables` directly (wall-clock lobby expiry); `aim_target`/`click_select`/`tab_targeting` write PlayerTarget+CurrentTarget+GameVariables+GameEvent.
- `dialogue_tick_system` reads UiEvent+GameEvent: putting its sim half under a set run_if = gated message reader.
- `drain_spawn_queue_system` is one monolith (players, cameras, nameplates, stat UI); GLB colliders depend on scene instancing → "logical spawn incl. collider" split is a large refactor, not a sub-bullet.

**Round-3 review (2026-11-19, plan rev 3 at 9084c75; verdict: Phases 1-2 Ready, 3-4 need more design):**
- `keep_messages_flowing_while_held` only re-arms `should_update = Ready` (sim_clock.rs:106) -> still max one flip/frame, so "redundant but harmless" next to a control tick is TRUE. But its doc invariant "FixedUpdate readers only read FixedUpdate-written messages" breaks for ANY capture->FixedUpdate message while no control tick exists (e.g. a Phase-3 BackendReply applier): lost after 2 held frames.
- `BackendReply` is already an enum in runtime/backend.rs:142 (adapter reply payload); `apply_reply` needs `&mut BackendHub` (in_flight, lobby gens) which the executor also writes -> a capture/apply split shares hub state.
- cursor_capture.rs: with no runner report (native) it sets `lock.locked = true` on the click itself; action_bar.rs:283-291 relies on running BEFORE it so the capturing click does not fire a mouse-button slot. Moving cursor_capture ahead of action_bar_input (capture layer) breaks that.
- real_pause.md:109/116 explicitly: `target_auto_clear_system` must NOT be gated and a condition on `GameStreamSet::Targeting` is forbidden for that reason.
- PendingJoinGamepad's consumer is the JoinPlayer executor arm (rule-driven), not default_panel_actions; today's `assign_handoff` end-of-Execute stale-clear (panel_defaults.rs:143-151) is already the right per-tick FIFO pattern.
- Bevy 0.18 single_threaded executor evaluates set conditions lazily at first member (single_threaded.rs:83-105); `FixedFirst`/`FixedLast`/`RunFixedMainLoopSystems::BeforeFixedMainLoop`/`resource_equals` all exist. Rapier nests RapierTransformPropagateSet/RapierBevyComponentApply in SyncBackend, so a SyncBackend run_if gates them too; `sync_removals` stays ungated in PostUpdate.

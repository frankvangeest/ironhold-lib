---
name: schedule-update-vs-fixedupdate
description: Which systems run in Update vs FixedUpdate, and the cross-schedule GameEvent timing hazard for any capability that must react to pipeline-emitted events
metadata:
  type: project
---

The interpreter chain runs in **Update**, NPC/physics AI runs in **FixedUpdate**. This split is the #1 timing hazard for any feature that wants a FixedUpdate capability to react to an event produced by the Message→Action pipeline.

**Update schedule** (lib.rs ~line 189, one `.chain()`):
`message_interpreter_system` → `fsm_interpreter_system` → `entity_fsm_interpreter_system` → `action_executor_system` → stat recompute/threshold → `drain_spawn_queue_system` → particle/decal drains. So `Action::EmitEvent` (and every action) is dispatched in **Update**.

**FixedUpdate schedule** (lib.rs ~line 208, one `.chain()`):
`input_translator_system` → `player_movement_system` → `collectible_system` → `trigger_zone_system` → `npc_behavior_system`.

**GameEvent is a Bevy message** (`add_message::<GameEvent>` in lib.rs ~129). Messages use double-buffering: an event written this frame survives ~2 frames before being dropped, and a `MessageReader` only sees events written since its own last run. FixedUpdate may run 0, 1, or many times per Update frame depending on accumulated time. So an event written in Update *will* be visible to a FixedUpdate reader, but **on a later FixedUpdate run, not synchronously**, and the per-frame ordering between the Update writer and the FixedUpdate reader is NOT guaranteed by schedule placement. You cannot order a FixedUpdate system "after action_executor_system" — they are in different schedules; `.after()` across schedules is meaningless.

**Correct pattern for FixedUpdate reaction to a pipeline event:** do NOT try to make a FixedUpdate system `MessageReader<GameEvent>` and rely on same-tick ordering. Either (a) have the capability's own FixedUpdate system own the detection (read the state it needs directly), or (b) accept a 1-tick latency and document it — a one-FixedUpdate-tick delay (≤16ms) is imperceptible for AI aggro. The contrast: `interactable_system` and `tick_delayed_events_system` are deliberately placed in **Update** `.before(message_interpreter_system)` precisely so all three interpreters see their GameEvents same-frame. A FixedUpdate emitter (`npc_behavior_system` writing `npc.player_reached`) is read by the Update interpreters on the next Update — that latency is already accepted in the codebase.

**Combat damage flow (3rd_person_game_demo):** `action_bar_input_system` (Update) reads the pressed digit, substitutes `{target}` from `CurrentTarget` into the *slot's* `do_actions` (where `ModifyStat(key:"{target}.health")` actually lives), pushes them to ActionQueue, and emits `action_bar.activated:N`. The `state_machine.ron` `action_bar.activated:N` handlers are cosmetic only (combat_status text). So the live target id is known in the action bar system at press time — the natural place to emit an attack/aggro signal, not the FSM handler.

# Feature: Gameplay-Logic Pipeline and Timers on the Fixed Tick

_Status: Draft (written autonomously 2026-10-02 while Frank was away; nothing here is plan-reviewed yet, and the pause decision in Open question 1 needs Frank before Phase 3)_
_Planned at: `a91a080` (2026-10-02)_

Backlog item: Beta 0.5 ▸ **"Move the gameplay-logic pipeline and its timers onto the fixed tick"**. Builds on
`deterministic_fixed_timestep.md` v1 (Done) and **D3** (`gameplay_pipeline_system_sets.md`, Ready: it
explicitly hands the `FixedUpdate` re-homing of its sets to this plan). Sibling items: D1/D2 (map order),
D4 (`SpawnId` tie-breaks), D5 (two-`App` smoke test), "Cross-platform determinism harness" (v2). Source audit:
`planning/investigations/hashmap_iteration_order_audit.md` (findings A, B, F, I).

## Phases

| Phase | Backlog item | Size | Status | Completed |
|---|---|---|---|---|
| 1 | `SimTick` counter + timers and stat ticks onto `FixedUpdate` | S-M | Queued | - |
| 2 | `SimInput` edge latch for fixed-tick input readers (fixes lost/duplicated presses) | M | Queued | - |
| 3 | Re-home D3's `GameplaySet`s into `FixedUpdate`; UI capture before the tick; paused control tick | L | Queued | - |
| 4 | Leftovers: spawn throttle, behavior activation, click-select resolver, harness hooks, docs | M | Queued | - |

Each phase merges and ships on its own. Phases 1 and 2 are valuable even if 3 never lands.

## What
Everything that decides what *happens* in a game becomes a function of the fixed tick, not of the render
frame: delayed events, cooldowns, despawn timers, stat regen and modifier expiry, dialogue auto-advance, and
finally the Message -> Interpreter -> Action -> Executor chain itself. A designer authors nothing
differently (`delay_secs`, `cooldown`, `duration` stay in seconds). Observable result: the same inputs
produce the same events on the same tick at 30, 60 and 144 fps, which is the precondition for replay and for
lockstep (`networking_multiplayer.md` Form 1). Purely cosmetic systems stay on the render clock.

## Why
Fixed-timestep v1 moved physics-adjacent systems only. Verified at `a91a080`:
- The fixed chain is `gamepad_bind -> input_translator -> player_movement -> view_box_clamp -> collectible ->
  trigger_zone -> npc_behavior -> motion -> mark_dirty_trees`, `.before(PhysicsSet::SyncBackend)`
  (`lib.rs:312-322`). Everything else that changes game state is `Update`, driven by `Time<Virtual>` deltas:
  `tick_delayed_events_system` (`lib.rs:751-767`, `*remaining -= dt`), `cooldown_tick_system`
  (`action_bar.rs:107-114`), `despawn_timer_system` (`despawn_timer.rs:46-55`, also pushes `ActionQueue`),
  `stat_modifier_system`/`stat_regen_system` (`stats.rs:8`, `:79`), `dialogue_tick_system` auto-advance
  (`dialogue.rs:201-202`), and the four interpreter/executor systems (`lib.rs:257-270`).
- Two machines at 60 and 144 fps therefore expire a 2.5 s cooldown on different ticks relative to physics, and
  a 144 fps machine runs the interpreter 2.25x more often than it steps physics. Bit-identical Rapier output
  does not help: the *inputs* to the next tick differ.
- The harness (v2) would currently report divergence at the first delayed event or cooldown, regardless of
  Rapier. This plan makes the harness measure the whole simulation.

## Findings (verified against code and vendored Bevy 0.18.0)
1. **`Res<Time>` inside `FixedUpdate` is already `Time<Fixed>`.** `run_fixed_main_schedule` overwrites the
   generic `Time` with `Time<Fixed>` before each tick (`bevy_time-0.18.0/src/fixed.rs:243-255`). Moving a
   timer system into `FixedUpdate` therefore makes `time.delta_secs()` equal `1/FIXED_TICK_RATE` (64 Hz,
   `physics.rs:12`) with **no change to the system body**. Constant `dt` makes f32 subtraction bit-deterministic.
2. **Frame order is `First, PreUpdate, StateTransition, RunFixedMainLoop (FixedMain x N), Update, PostUpdate`**
   (`bevy_app main_schedule.rs:104`, `bevy_time lib.rs:80-96`). `FixedUpdate` runs *before* `Update` in a
   frame. `ButtonInput`/`Interaction` are fed in `PreUpdate`, so fixed systems see this frame's edges, but an
   `Update`-written message is first seen by the *next* frame's tick.
3. **Message double-buffering is already fixed-loop-aware.** `Messages::update` is gated: `FixedPostUpdate`
   runs `signal_message_update_system`, and `message_update_system` (in `First`) only flips buffers after that
   signal (`bevy_time lib.rs:92-96`, `bevy_ecs message/update.rs:16-56`; initial state `Waiting`). So a
   message written in `Update` survives 0-tick frames, and a `MessageReader` cursor delivers it **exactly
   once** across 2-tick frames. No message is dropped or doubled by moving *readers* into `FixedUpdate`; the
   existing `FixedUpdate` writers (`collectible`, `trigger_zone`, `npc_behavior`, `player.rs:451`) already
   rely on this in the other direction. Hazard: a reader behind a `run_if` that stays false for 2 flips loses
   messages, so the interpreter must never be run-gated off (see pause, Q1).
4. **Polled state is not protected like messages.** `ButtonInput::just_pressed` is cleared each `PreUpdate`.
   Fixed-tick readers (`input.rs:372-381` jump/run, and `interactable.rs:56`, `action_bar.rs:186`,
   `targeting.rs:202/297` once moved) lose a press on a 0-tick frame and see it twice on a 2-tick frame.
   Expected pre-existing bug for the jump/run reads in `input_translator_system` (about 56 % of frames have 0
   ticks at 144 Hz; about 4 per second have 2 ticks at 60 Hz, `lib.rs:289-296`). Code-reading only, **not yet
   reproduced** - Phase 2's first task is a 0-tick/2-tick regression test that confirms it.
5. **`Changed<Interaction>` is a partial latch.** `button_system`/`icon_button_click_system`
   (`lib.rs:501-526`, `:545-560`) and the action-bar click edge (`Ref<Interaction>::is_changed`,
   `src/CLAUDE.md` "Mouse-click action-bar slots") compare against the system's *own* last run. In a
   fixed-rate system that spans several frames, so a press + release between two ticks collapses to the final
   `None` and the click is lost. Click capture must stay a once-per-frame system.
6. **Tests already pin exactly one tick per `app.update()`**: `tests/support/mod.rs:79` sets
   `TimeUpdateStrategy::ManualDuration(1/FIXED_TICK_RATE)`. Only 5 of 25 test files override it
   (`corpse_loot_interact`, `local_coop`, `player_slope_jump`, `prop_ground_veto`, `wall_friction`). Timer-based
   tests (`action_tests`, `entity_logic_tests`, `corpse_loot_interact_tests`, `local_coop_tests`) already see
   `dt = 1/64` in `Update`, so moving timers changes no numbers. The risk is **ordering within one
   `app.update()`**, not dt (see Approach 5).
7. **Pause.** `real_pause.md` (Queued, not implemented) freezes `Time<Virtual>` and requires the interpreter
   and executor to keep running while paused. A frozen `Time<Virtual>` means *zero* `FixedUpdate` ticks
   (`fixed.rs:244-247`: overstep accumulates the virtual delta). Moving the interpreter into `FixedUpdate`
   directly conflicts with that plan's lever; a Resume button would never be processed. This is the single
   biggest design interaction (Approach 6, Open question 1).
8. **`Time<Virtual>::max_delta` bounds catch-up**: default 250 ms = at most 16 ticks in one frame; the
   project override `max_frame_delta_secs` allows as little as 2 ticks (`project.rs:229-269`). After a clamp
   the sim runs in slow motion, which is deterministic (ticks are still whole).
9. `drain_spawn_queue_system` throttles with `SPAWNS_PER_FRAME = 2` (`entity_spawner.rs:343-363`), a
   *render-stall* guard that also decides which tick an entity exists on. `npc_hit_relay_system` stays in
   `Update` only to avoid "cross-schedule double-buffer complexity" (`lib.rs:275-278`); finding 3 removes that
   reason.

## Approach

### 1. What ticks on the fixed clock (decision)
- **Fixed tick (sim):** delayed events, cooldowns, despawn timers, stat modifier/regen/effective/threshold,
  dialogue auto-advance, the interpreters, `flush_pending_intent`, the executor and its drain tail
  (`drain_spawn_queue`, pools), `action_bar_input`, `interactable`, tab-targeting, `dialogue_tick`,
  `resolve_pending_behaviors`, `npc_hit_relay`, `update_player_speed`.
- **Render rate (cosmetic, unchanged):** `damage_popup`, `fading_light`/`fading_decal`, `clear_pool_on_scene_unload`,
  camera chain, `animation_resolver`/`animation_playback`, nameplates/world labels, stat bars and radar,
  `inventory_ui`/`container_ui` (they only mirror state; they lose D3's `.after(action_executor_system)` edge,
  which cannot cross schedules, and at worst lag one frame), particle simulation, audio.
- **Scene/asset I/O (`spawn_scene_v2`, `check_project_loaded`, preloads): stays `Update`.** It is
  asset-load-driven and cannot be tick-exact (audit finding B). Its `SceneEvent`s enter the tick stream via
  finding 3. `LoadScene`'s ordering hazard (`lib.rs:219-226`) disappears: the executor's deferred `SceneHandleV2`
  commands are applied at the end of the `FixedMain` schedule, before `Update`'s `spawn_scene_v2` runs. (Task:
  verify with a test; if wrong, add an explicit `ApplyDeferred`.)

### 2. Clock representation (decision): constant-`dt` f32 plus an explicit `SimTick(u64)`
- Add `FIXED_DT = 1.0 / FIXED_TICK_RATE` next to `FIXED_TICK_RATE`, and a `SimTick(u64)` resource incremented
  by the first system of each tick.
- Timers **keep counting seconds** with the constant `dt`. `SimTick` is for tick-stamping (latched inputs,
  logs, harness state hashes, replay), not for timer arithmetic.
- *Rejected: integer tick deadlines for every timer* (`due_tick = SimTick + ceil(secs * 64)`). Cleaner to hash
  and immune to float drift, but it changes authored semantics (rounding up to 15.6 ms), changes `LiveStat`,
  `CooldownMap` and `DelayedEventQueue` types and every test that reads them, for no determinism gain:
  `1/64` is a power of two, so decrementing by it is exact in f32 for realistic magnitudes. Revisit only if the
  harness shows drift.
- *Rejected: `Time<Virtual>` or `Time<Real>` deltas in sim.* Frame-rate dependent by definition.
- *Rejected: using `Time<Fixed>::elapsed()` as the tick counter.* Works (exact nanoseconds) but is a `Duration`
  that anything can `set_timestep` under; an explicit integer survives a future slow-motion feature and is the
  natural key for recorded inputs.

### 3. Crossing the Update -> fixed boundary (decision)
Three kinds of input, three mechanisms. Rule: **everything that touches wall-clock, the window, GPU or the
camera is a "capture" system; capture systems only translate and never read sim state.**
1. *Messages* (`UiEvent`, `GameEvent`, `SceneEvent`): nothing to build; finding 3 gives exactly-once delivery
   on 0- and 2-tick frames.
2. *Polled edges* (`ButtonInput`, `Gamepad::just_pressed`, mouse buttons): new `SimInput` resource. A capture
   system runs **once per frame** in `RunFixedMainLoopSystems::BeforeFixedMainLoop` and *unions* this frame's
   edges into it (so a 0-tick frame accumulates, a press+release survives). Fixed systems read edges through a
   thin `SimInput` SystemParam with the same API as `just_pressed(..)`. A `clear_sim_input_edges` system runs
   **last** in each tick, so only the first tick of a multi-tick frame sees an edge. *Level* state (held keys,
   stick axes) is sampled live at tick time. `SimInput` is also the replay/network seam: it answers the audit's
   open question "will lockstep inject through `ButtonInput`" - no, through `SimInput`.
3. *UI interaction edges* (`Changed<Interaction>`, slot clicks): `button_system`, `icon_button_click_system`,
   `global_input_system`, `unclaimed_gamepad_trigger_system` and the slot-click detection move to
   `BeforeFixedMainLoop` and write messages (kind 1) or `SimInput` slot-click edges (kind 2). They run after
   `ui_focus_system` (`PreUpdate`) and before the tick.
- *Rejected: `InputQueue` of typed commands drained by the first tick.* It is the right end state for network
  play but duplicates `Messages` for UI events; `SimInput` + messages is a smaller step and converges on a
  command stream later (Phase 4 hook).
- *Rejected: just reading `just_pressed` in `FixedUpdate` as today.* Finding 4.
- *Rejected: running `Update` capture then waiting one frame.* Costs a frame of latency (below).
- Mouse **click-to-target** needs camera + cursor (render state). It stays a capture system that resolves the
  click to a `SpawnId` and stores `SimInput.target_click: Option<String>`; the sim applies it at tick time.
  Same idea for any future camera-dependent input.

### 4. Latency and 0-/2-tick semantics (decisions)
- Capture runs immediately before `FixedMain` in the same frame, so a click or key is visible to the **first
  tick of that same frame**: no extra frame. Worst-case added delay is the wait for the next tick when the frame
  has 0 ticks: <= 1/64 s = 15.6 ms (mean 7.8 ms at 144 Hz, about 0 at 30 fps where every frame has >= 2 ticks).
  Today a click at 144 fps is acted on that frame (about 7 ms) but the *resulting* `LoadScene`/state change is
  not visible until the next render anyway; the new worst case is on the order of one 64 Hz physics tick, the
  same granularity the player already feels for movement. Document it; measure in the playtest.
- **0 ticks in a frame:** nothing is consumed and nothing is lost. `ActionQueue`, `PendingIntentActions` and
  `DelayedEventQueue` are resources and persist; messages and `SimInput` edges accumulate (finding 3, Approach 3).
- **N ticks in a frame:** each message is read once (cursor); each edge is seen by the first tick only; each
  tick fully drains `ActionQueue`, so actions never leak across ticks. Per-tick order is D3's order.
- Event latency inside the tick shrinks: D3's "carry-over from the previous frame's `Execute`" becomes
  carry-over from the previous **tick** (1/64 s, was 1 frame).

### 5. Tick layout (re-homing D3's sets; does not contradict D3)
D3's sets are reused unchanged and moved to `FixedUpdate` in `GameplaySchedulePlugin`; only membership edges
change. One tick, in order:
1. `SimTickSet::Begin`: `advance_sim_tick` (+ in Phase 3, the paused-control gate).
2. Existing movement chain, **split**: `gamepad_bind -> input_translator -> player_movement -> view_box_clamp`.
3. `GameplaySet::Emit`: `EmitSet::Ui`, `Game` = `Targeting -> ActionBar -> Interact -> **World** ->
   DelayedEvents -> Stats`, `Scene`, `DirectActions`, `Dialogue`. New member `GameStreamSet::World` =
   `collectible -> trigger_zone -> npc_behavior` (today the tail of the movement chain). D3's Approach 3 bullet
   "(2) events from each `FixedUpdate` tick" becomes an in-stream position; D3's docs wording is updated by
   Phase 3, not before.
4. `GameplaySet::Interpret -> Execute -> PostExecute` (incl. `stat_threshold`, now seeing same-tick regen, which
   resolves D3's deferred decision D-a).
5. `motion_system -> mark_dirty_trees`, then `PhysicsSet::SyncBackend/StepSimulation/Writeback`.
   The pipeline sits **before** `SyncBackend`, so `ResetToSpawn`/teleport/spawn effects reach Rapier the same
   tick. `CollisionEvent`s still come from the previous tick's step (unchanged one-tick delay).
6. `clear_sim_input_edges` (last).
- Phase 1 and 2 use temporary slots (a `FixedTimerSet` placed before step 3). D3's `DelayedEvents` and `Stats`
  stream entries and the source-scan allowlist (`schedule_order_tests`) move out of `Update` in Phase 1; if
  D3 has not shipped yet, implement Phase 1 against the final layout instead.
- `GameplaySchedulePlugin` must keep the **exact registration spot** and ordering comments (D3 N5): WASM
  ties break by insertion order.

### 6. Pause and `Time<Virtual>` (decision with an open question)
`real_pause.md` freezes `Time<Virtual>`, which stops `FixedMain` (finding 7). Recommended resolution: keep the
Virtual freeze (it still stops sim timers, physics and animation for free) and add a **paused control tick**:
a system in `BeforeFixedMainLoop` that, when `Time<Virtual>::is_paused()`, calls
`Time::<Fixed>::accumulate_overstep(period)`, so exactly one `FixedMain` runs per rendered frame. A single
`SimActive` run condition gates the sim-only sets (`World`, `DelayedEvents`, `Stats`, movement, physics, `motion`,
`SimTick`); the `Ui`/`Interpret`/`Execute`/`Scene` sets run in the paused tick. One system instance means one
message cursor, so there is no double delivery - which a second "paused" schedule would cause (each
`MessageReader` has its own cursor and would replay buffered messages). Rejected alternatives:
- *Second schedule for paused control:* duplicated cursors, finding above.
- *No Virtual freeze; gate everything with a `SimPaused` flag:* cleanest for replay (pause becomes a recorded
  command) but animation, particles and audio then need separate pause levers, undoing real_pause's main win.
Needs a spike (Phase 3, first task) because `accumulate_overstep` on a paused clock is a supported but untested
use; fallback is the `SimPaused` flag. `real_pause.md` needs a one-paragraph amendment when this is accepted.

### 7. What stays non-deterministic after this plan
Documented in `docs/40_determinism_and_networking.md` and asserted as "expected divergence" in the harness:
rendering/animation pose, audio, particle simulation; **asset-load completion tick** (`SceneEvent::Ready`,
`BehaviorHandle`, dialogue assets, GLTF readiness - audit finding B; harness answer: hold tick 0 until Ready and
stamp the tick, or record load-complete as an input); camera-derived input (click-to-target); wall-clock-fed
local hardware heuristics (`stable_secs`/`stuck_secs` gamepad debounce, `input.rs:196-198`); `Entity` indices
(never tie-break on them in sim, per the audit); query-order dependencies owned by D4. The harness (v2) samples
a per-tick hash of {`SimTick`, `LogicState`, `GameVariables`, `ActionQueue` length, `CooldownMap`,
`DelayedEventQueue`, `LoadedStats`, `SpawnRegistry` keys, rigid-body transforms} and reports the first diverging
tick; Phase 4 adds the hash hook.

### 8. WASM / performance
- Per-tick cost: these systems already run once per frame and have early-outs (`cooldown_tick` returns on an
  empty map). At 60 Hz render the load is about unchanged, at 144 Hz it **falls** (64 runs/s vs 144), at 30 fps
  it doubles per frame (2 ticks). Catch-up bursts after a stall are bounded by `max_delta` (<= 16 ticks).
- Watch items: `entity_fsm_interpreter_system` (iterates behavior entities), `stat_*` loops over every
  `StatMap`, `action_bar_input` (query over slots), `drain_spawn_queue` burst. Measure with the diagnostics HUD /
  Tracy on `3rd_person_game_demo` and `local_coop_demo` at forced 30 fps in Phase 3.
- `SPAWNS_PER_FRAME` (Phase 4): becomes a per-tick cap so entity existence is tick-deterministic. Burst risk:
  after a stall 16 ticks x 2 spawns could land in one frame and re-create the WebGPU compile stall the cap
  exists to prevent. Proposal: `SPAWNS_PER_TICK = 1` plus the existing `max_frame_delta_secs` lever; measure
  wave-spawn duration (a 20-enemy wave: 0.3 s at 64 Hz vs 0.17 s today at 60 fps / cap 2).

### 9. Tests: blast radius and migration
- ~707 tests, ~1018 `app.update()` calls in 20 of 25 files. Because the harness already runs one tick per
  update (finding 6), dt-dependent tests are unaffected. The break class is **same-`update()` chains**: today
  `Update` emitters -> interpreter -> executor all run in one call; if emitters stay in `Update` while consumers
  move to `FixedUpdate` (which runs *before* `Update`), a one-call chain stretches to two.
- Mitigation by design: Phase 3 moves the emitters to `BeforeFixedMainLoop` / `FixedUpdate`, so
  "set input -> `app.update()` -> assert" keeps working in one call. Only tests that inject into `Messages<..>`
  or set `Interaction`/`ButtonInput` *before* `update()` (the common style) are unaffected; tests that
  depend on an `Update`-only system's output feeding the pipeline in the same call need one extra `update()`.
- Expected edits: single digits to low tens of tests (candidates: `entity_logic_tests`, `action_tests`,
  `local_coop_tests`, `ui_tests`, `fsm_tests`, `scene_lifecycle_tests`), done by a mechanical helper
  `support::tick(&mut app, n)`. Add tests: 0-tick frame, 2-tick frame, 4-tick burst, 144 Hz cadence
  (`ManualDuration(1/144 s)`) for each of: press, click, delayed event, cooldown, `ActionQueue` drain; plus a
  two-`App` determinism test running at 30 and 144 Hz cadences and comparing event logs per `SimTick` (this is
  D5's test extended; coordinate, do not duplicate).
- Run per-file (disk rule); `cargo check -p ironhold_cli` each phase.

## Tasks
**Phase 1 (S-M)**
- [ ] `FIXED_DT`, `SimTick`, `advance_sim_tick`; derive from `FIXED_TICK_RATE`
- [ ] Move to `FixedUpdate` (no body changes): `tick_delayed_events`, `cooldown_tick`, `despawn_timer`,
      `stat_modifier -> stat_regen -> stat_effective_value`, dialogue timer; order before `SyncBackend`
- [ ] Update D3 allowlist/membership and the `lib.rs:242-243` comment; `src/CLAUDE.md` timer notes
- [ ] Tests: delayed event / cooldown / modifier expire on the same `SimTick` under 30/60/144 Hz cadences
**Phase 2 (M)**
- [ ] Reproduce finding 4 (lost jump on 0-tick frame, doubled on 2-tick frame) as failing tests first
- [ ] `SimInput` resource + SystemParam + `BeforeFixedMainLoop` capture + end-of-tick clear
- [ ] Migrate `input_translator`, `gamepad_bind`, then `interactable`/`action_bar_input`/tab-targeting reads
- [ ] Docs: `docs/40` "input seam"
**Phase 3 (L)**
- [ ] Spike: paused control tick (`accumulate_overstep`); decide with Frank (Q1); amend `real_pause.md`
- [ ] Move D3 sets to `FixedUpdate`; add `GameStreamSet::World`; split movement chain; keep registration spot
- [ ] Move UI capture systems to `BeforeFixedMainLoop`; `npc_hit_relay` into `PostExecute`; verify deferred
      `SceneHandleV2` ordering vs `spawn_scene_v2`
- [ ] Update `schedule_order_tests` (graph moves to `FixedUpdate`), `tests/support`, migrate broken tests
- [ ] Docs: `docs/30` "Same-frame event order" -> "Same-tick event order"; `src/CLAUDE.md` pipeline section
- [ ] Reviews: alignment, system-architect, debug-detective, wasm-perf-reviewer, ux-gamedesigner-reviewer
**Phase 4 (M)**
- [ ] `SPAWNS_PER_TICK`; `resolve_pending_behaviors` activation tick-gating; click-select -> `SpawnId` capture
- [ ] Harness state-hash hook and "expected divergence" list; `docs/40` rewrite; D4 tie-breaks coordinated

## Playtest checklist
- `3rd_person_game_demo` at native 144 Hz, 60 Hz and a forced 30 fps (throttled browser tab on web): jump, skill
  keys, mouse-click skill slots, tab-target, interact - none feel laggy or dropped; mash jump at 144 Hz.
- Cooldown overlay and monster respawn (60 s delayed event) land on the same tick on both rates.
- Pause overlay: Resume click, Esc, Resume+Esc same frame, pause during a delayed event (must not fire),
  `LoadScene` while paused (real_pause cases).
- `local_coop_demo`: both players fire skills in one frame; hot-join with gamepad; 4-way split.
- Wave spawn timing (Phase 4), death/respawn flow, dialogue auto-advance, chest/loot panel.
- `python test_web.py` baselines unchanged; native vs web event log (`SimTick`-stamped) identical for a scripted run.

## Open questions (for Frank)
1. **Pause mechanism (blocks Phase 3).** Paused control tick via `accumulate_overstep` (recommended, keeps
   real_pause's Virtual freeze) vs. a `SimPaused` gate with no Virtual freeze (cleaner for replay, needs separate
   animation/particle/audio pause)? Either amends `real_pause.md`.
2. **Accept the <= 15.6 ms click-to-effect granularity** (UI-driven actions act on the next tick, not the next
   render frame)? If not, the only alternative is keeping UI-initiated `FSM` transitions in `Update` and
   splitting the interpreter, which breaks the single-order guarantee - not recommended.
3. **Spawn throttle:** is wave spawning 2x slower (cap 1 per tick) acceptable, or keep 2 per tick and accept
   catch-up bursts?
4. **Phase order vs D3/D1/D2:** this plan assumes D3 ships first and D1/D2 (map order) before or with Phase 3.
   OK, or should Phase 1 go first because it is independent?

## Acceptance criteria
- Given identical input streams stamped by `SimTick`, when run at 30, 60 and 144 fps cadences, then every
  timer-driven event (delayed, cooldown, despawn, regen, modifier expiry, dialogue advance) fires on the same
  `SimTick` and the interpreter receives events in D3's order at the same tick.
- Given a key/click/gamepad press on a frame with 0 ticks, then it is applied by the next tick exactly once;
  given a frame with 2+ ticks, then it is applied once, by the first tick.
- Given `Time<Virtual>` is paused, then no sim timer advances, `SimTick` is frozen, and Resume/UI transitions
  still process (per Q1's outcome).
- Cosmetic systems are unchanged; no new frame of UI latency beyond the <= 1 tick wait; 30 fps and 144 fps
  frame cost within agreed budget on `3rd_person_game_demo`.
- The existing suite passes after the documented mechanical migration; `ironhold_cli` checks; harness expected-divergence
  list is documented in `docs/40_determinism_and_networking.md`.

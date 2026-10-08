# Feature: Gameplay-Logic Pipeline and Timers on the Fixed Tick

_Status: Draft — plan-review 2026-10-02: needs more design work (see "Plan-review" section at the end)_
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
   `capabilities/CLAUDE.md` "Mouse-click action-bar slots") compare against the system's *own* last run. In a
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
Documented in `docs/dev/40_determinism_and_networking.md` and asserted as "expected divergence" in the harness:
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
- [ ] Update D3 allowlist/membership and the `lib.rs:242-243` comment; `capabilities/CLAUDE.md` timer notes
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
  list is documented in `docs/dev/40_determinism_and_networking.md`.

## Plan-review (2026-10-02)

Run by `/plan-review` while Frank was away: system-architect and ux-gamedesigner-reviewer, in parallel, read-only, claims checked against the code. **Combined verdict: needs more design work** — the blocking items below must be folded into this plan (and Frank's answers to the open questions recorded) before it can be marked Ready. The reports are reproduced verbatim (headings demoted one level).

### System-architect review

## Plan review: gameplay_fixed_tick_pipeline.md (system-architect)

Reviewed at `34803b1` (integration), plan `Planned at: a91a080` (exists). Verified against `bevy_time-0.18.0`,
`bevy_ecs-0.18.0`, `bevy_app-0.18.0` in `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`. D3 is **not
shipped** (`runtime/schedule.rs` does not exist; `lib.rs:217-334` still has the ad-hoc edges).

#### Verdict

**Needs more design work.** The direction is right and most of the Bevy findings are correct (see the checks
below), but the plan does not hold together in five places: the pause gate, the Execute tail, edges that
cross schedules, the timing of Phase 2's `SimInput` consumers, and the event order during Phase 1. Each one
would produce a real bug if implemented as written. The core idea needs no rework. All five can be fixed
inside the plan file in one revision.

**Verified claims (correct):**
- (a) `Res<Time>` inside `FixedUpdate` is `Time<Fixed>`: `bevy_time fixed.rs:240-256`
  (`run_fixed_main_schedule` overwrites generic `Time` per tick and restores `Time<Virtual>` afterwards).
- (b) Message delivery across the boundary: `signal_message_update_system` runs in `FixedPostUpdate`
  (`bevy_time lib.rs:91-95`, initial `Waiting`). `message_update_system` only flips when `Ready` and resets to
  `Waiting` (`bevy_ecs message/update.rs:27-55`). So buffers flip **at most once per frame**, and only at the
  `First` that follows a frame with at least one tick. That gives:
  - Update writer, FixedUpdate reader: the second flip needs a tick that runs *after* the write, so a
    fixed-tick reader always sees the message, through 0-tick frames too. 2-tick frames: the cursor gives
    exactly-once delivery.
  - FixedUpdate writer, Update reader: `Update` runs every frame and there is at most one flip per frame, so
    delivery is exactly once.
  - `BeforeFixedMainLoop` writer, FixedUpdate reader: same as an Update writer, but seen in the same frame.
  - Caveat (correct in the plan): a reader that does not run while two flips happen loses messages. The plan
    then creates this case itself (B1).
- (e, partially) The deferred `SceneHandleV2` claim holds. Every schedule run applies its final deferred
  commands (`apply_final_deferred: true` by default, `executor/single_threaded.rs:171,192`,
  `multi_threaded.rs:299,397`). So `FixedUpdate`'s commands are applied before `Update`'s `spawn_scene_v2`. No
  `ApplyDeferred` is needed. Keep the confirming test.
- (f) `tests/support/mod.rs:79-81` pins `ManualDuration(1/64)`, and `1/64` is exact in both f32 and `Duration`.
  But see N1: the **first** `app.update()` runs 0 ticks.
- `RunFixedMainLoopSystems::BeforeFixedMainLoop` exists (`bevy_app main_schedule.rs:404-482`). `RunFixedMainLoop`
  runs its systems without parallelism, which suits a capture chain.
- Finding 4 mechanism is real: `runtime/input.rs:373-387` reads `just_pressed` in `FixedUpdate` (the plan cites
  `input.rs` without the `runtime/` prefix). `interactable.rs:56/61`, `action_bar.rs:186-188` and
  `targeting.rs:202` read edges in `Update` today.

#### Blocking

**B1. Approach 6's paused control tick gates the wrong set of systems. Three bugs follow.**
- The gated list is `World`, `DelayedEvents`, `Stats`, movement, physics, `motion`, `SimTick`. Everything else
  runs in every paused control tick with `Res<Time>` = `Time<Fixed>`, so `delta_secs() = 1/64` for each
  **rendered frame** (`accumulate_overstep(period)`, then `expend()` advances by one timestep). Systems that
  consume dt but are not gated:
  - `cooldown_tick_system` (`action_bar.rs:107-114`, in `GameStreamSet::ActionBar`)
  - `despawn_timer_system` (`EmitSet::DirectActions`)
  - dialogue auto-advance (`dialogue.rs:202`, `EmitSet::Dialogue`)

  These keep running while paused, at 2.25x speed on a 144 Hz display. `real_pause.md` also requires
  `action_bar_input`, `interactable`, `tab_targeting` and `click_select` to be gated, and they are not in the
  list either.
- **A Resume in the middle of a tick runs a physics step that has no `SimTick`.** The executor unpauses during
  `Execute`. If `SimActive` is a run condition that reads `Time<Virtual>::is_paused()`, then `motion` and
  `PhysicsSet::*`, which sit after `Execute` in the same tick, now run, while `advance_sim_tick` (in `Begin`) did
  not run. That breaks tick-stamping and the harness hash. The reverse case: a pause executed in tick k of an
  N-tick frame must turn ticks k+1..N into control ticks. That only works if the gate is evaluated per tick, not
  per frame.
- **Gated message readers lose events.** Control ticks still run `FixedPostUpdate`, which sends
  `signal_message_update_system`, so buffers flip every paused frame. `collectible_system`
  (`collectible.rs:27`) and `trigger_zone_system` (`trigger_zone.rs:33`) read `MessageReader<CollisionEvent>`.
  If they are gated, the collisions from the last step before the pause are dropped after two paused frames.
  The visible results are a lost pickup, or a trigger zone that never gets its enter/exit and stays latched.
  This is exactly the hazard the plan names in Finding 3.
- **Fix:**
  1. Invert the rule to a **whitelist**: only `Interpret`, `Execute`, `PostExecute` and the end-of-tick clear
     run in a control tick. Everything else is gated.
  2. Latch `SimActive` once, in `SimTickSet::Begin`, and use `resource_equals` on the latch, never a live
     `is_paused()` read.
  3. Every message reader inside a gated set must be fed by an ungated drain into a persistent queue. This is
     the `NpcHitQueue` pattern: a `CollisionEventQueue` filled by an ungated system.
  4. Make the gate **real_pause's single clock owner** (`GamePaused`, shared with `static_scene_mode.md` and
     `pause_on_focus_loss`), not a third flag next to `GamePaused` and `Time<Virtual>::is_paused()`.
  5. Spike an alternative to `accumulate_overstep`: when paused, an exclusive `BeforeFixedMainLoop` system
     calls `world.run_schedule(FixedMain)` directly. Generic `Time` is then `Time<Virtual>` (paused, delta 0),
     so every dt consumer stops by itself as a second line of defence. `Time<Fixed>::elapsed`/`overstep` stay
     untouched (`accumulate_overstep` advances `Time<Fixed>::elapsed` one period per paused frame). It is
     still one system instance, so there are no duplicated cursors.

  Add an acceptance criterion for all three bugs: no cooldown, despawn or dialogue timer moves during a pause;
  Resume runs no physics step in the same tick; a collision on the tick before the pause is still processed
  after Resume.

**B2. Re-homing D3's `Execute` set moves render-rate systems onto the fixed tick, and the spawn-throttle change
lands a phase too early.**
- D3 keeps "the existing drain/pool/decal tail (chain segment kept as is)" in `GameplaySet::Execute`
  (`gameplay_pipeline_system_sets.md:94`; `lib.rs:257-270`). That tail contains:
  - `simulate_pool_system` (`particle_renderer.rs:257`, particle integration with `Res<Time>`)
  - `rebuild_pool_meshes_system` (rebuilds meshes every frame)
  - `drain_particle_effects_system`, `spawn_decal_system`, `drain_dynamic_stat_ui_system`

  Approach 5 says D3's sets are "reused unchanged", and Approach 1 puts "pools" on the sim side while also
  saying "particle simulation" stays render-rate. The plan contradicts itself. If implemented literally,
  particles step at 64 Hz (visible stutter at 144 Hz, with no interpolation), are not simulated on 0-tick
  frames, and the mesh rebuild runs up to 16x in a catch-up frame on single-threaded WASM.
- `drain_spawn_queue_system` is in the same chain, so Phase 3 already turns `SPAWNS_PER_FRAME = 2`
  (`entity_spawner.rs:343,363`) into 2 per **tick**, up to 32 in a burst frame. That happens before Phase 4's
  cap decision (Open Question 3) is made.
- **Fix:**
  - Split `Execute` explicitly. On the fixed tick:
    `flush_pending_intent → action_executor → stat_effective_value(2nd) → stat_threshold → drain_spawn_queue`.
  - In `Update`, render-rate, draining persistent queues:
    `drain_dynamic_stat_ui → drain_particle_effects → simulate_pool → rebuild_pool_meshes → spawn_decal`.
    `fading_light`/`fading_decal` keep their `.after` edges (`lib.rs:271-272`).
  - Move the spawn-cap decision into Phase 3.
  - Recommended for the cap: tick-deterministic *logical* spawn (registry entry, components, collider) at a
    per-tick cap, plus a separate **per-frame** cap on render instantiation (SceneRoot/material attach). The
    WebGPU compile stall is driven by render attachment, not by sim existence. `SPAWNS_PER_TICK = 1` alone
    still allows 16 per frame after a stall, and the stall it produces feeds the next catch-up.

**B3. The layout requires sets and ordering edges to span schedules, and the edges that become dead are not
inventoried.**
- Approach 3 moves the `EmitSet::Ui` members (`button_system`, `icon_button_click_system`,
  `global_input_system`, `unclaimed_gamepad_trigger_system`) to `RunFixedMainLoop`/`BeforeFixedMainLoop`.
  Approach 1 keeps `spawn_scene_v2`/`check_project_loaded` (D3's `EmitSet::Scene`) in `Update`. Yet Approach 5
  lists `EmitSet::Ui` and `Scene` inside `GameplaySet::Emit` in `FixedUpdate`. A `SystemSet`'s configuration
  is per schedule, so these sets would be empty in `FixedUpdate`.
- Bevy accepts `.before/.after(fn)` against a `SystemTypeSet` that has no members in that schedule, without
  any warning. Today's edges that silently become no-ops:
  - `dialogue_tick.after(button_system)` (`lib.rs:329`)
  - `audio_state_system.before(fsm_interpreter_system)` (`lib.rs:251`). This one is not mentioned anywhere in
    the plan, and it exists so that `mute_on_start` is applied before `PlayMusicLoop` runs. Once the executor
    runs in the tick that precedes `Update`, this edge is inverted.
  - `click_select/tab/auto_clear.before(action_bar_input_system)` (`targeting.rs:65`), if one side moves before
    the other
  - `inventory_ui/container_ui.after(action_executor_system)` (`lib.rs:389-390`), which the plan does mention
  - `npc_hit_relay.after(action_executor_system)` (`lib.rs:278`)
  - D3's `audio_state_system.before(GameplaySet::Interpret)`
- **Fix:**
  - Add a **per-schedule layout table** (`RunFixedMainLoop`/`BeforeFixedMainLoop`, `FixedUpdate`, `Update`)
    that lists every set and every edge that crosses into a different schedule, with its new form.
  - Place `audio_state_system` explicitly. Recommended: `BeforeFixedMainLoop`. Otherwise prove with a test that
    `bevy_audio`'s `PostUpdate` playback start still sees the muted `GlobalVolume`.
  - Add a schedule test: every system-type ordering target in `Update`, `FixedUpdate` and `RunFixedMainLoop`
    resolves to at least one system in **that** schedule. This catches the silent dead edge. It belongs in
    `schedule_order_tests.rs`.
  - D3's structure and source-scan tests become multi-schedule; say so in the Phase 3 tasks.

**B4. Phase 2's `SimInput` is cleared at the end of every tick, so any consumer still in `Update` never sees an
edge, and Phase 2 plans to migrate consumers that are still in `Update`.**
- `clear_sim_input_edges` runs last in each tick, and `Update` runs after `FixedMain`. So on every frame with
  at least one tick, an `Update` reader sees an empty `SimInput`. Phase 2's task list migrates
  "`interactable`/`action_bar_input`/tab-targeting reads", but those systems stay in `Update` until Phase 3.
  Finding 4 itself says "once moved". Implemented as written, keyboard interact, skill keys and Tab would stop
  working on most frames.
- **Fix:** Phase 2 migrates only readers that already live in `FixedUpdate` (`input_translator`,
  `gamepad_bind`). The `Update` readers switch to `SimInput` in Phase 3, in the same commit as their schedule
  move. Add a debug assertion or test: a `SimInput` read from outside `FixedMain` panics in tests.
- Also specify the `SimInput` shape before coding:
  - Click-to-target as `Option<String>` cannot express "clicked empty space, clear this player's target"
    (`targeting.rs:256-259`) or *which* player acted (`targeting.rs:221-228`, cursor-to-viewport owner). It
    needs an enum such as `TargetClick::{Select{player, target}, Clear{player}}` keyed by player `SpawnId`.
  - Gamepad edges must be per-pad and per-player (`action_bar.rs:171-172` and `interactable.rs:61` read
    `BoundGamepad`). Key them by player `SpawnId`/`PlayerIndex` at capture time, not by `Entity`, so the struct
    is the replay seam the plan claims.
  - Slot clicks keyed by `(bar.id, slot_key)` (see the action-bar per-player keying memo: slot_key alone is
    not unique across bars).

**B5. Phase 1 on its own changes same-frame event order and leaves the fixed-tick `GameEvent` writers
unordered.**
- `FixedMain` runs before `Update`. Moving `tick_delayed_events` and the stat chain into `FixedUpdate` while
  targeting, the action bar and `interactable` stay in `Update` puts delayed and stat events **ahead of**
  same-frame input events. That inverts D3's accepted principle "input events first, then delayed events, then
  stat events" (`gameplay_pipeline_system_sets.md:113`). The FSM takes the first matching transition, so this is
  a behavior change.
- "a `FixedTimerSet` placed before step 3" names no edge against the existing `GameEvent` writers
  (`collectible`, `trigger_zone`, `npc_behavior`, `lib.rs:312-322`). Same-type `MessageWriter`s serialise in an
  executor-chosen order, which reintroduces in `FixedUpdate` the exact nondeterminism D3 removes from `Update`.
- "If D3 has not shipped yet, implement Phase 1 against the final layout instead" is not implementable: the
  final layout is defined in terms of D3's sets.
- **Fix:**
  - Make D3 a **hard prerequisite** of Phase 1 (it is Ready and nothing blocks it).
  - Chain Phase 1's timer set to an explicit position in the fixed chain: after `npc_behavior`, before `motion`,
    so the World events and then the timer/stat events come in D3's relative order.
  - State the interim input-vs-timer inversion in the plan and in `docs/30` as a Phase 1 behavior change, with
    one test, until Phase 3 restores the full order.
  - Say how `dialogue_tick_system` is handled in Phase 1. It is one system that combines the timer
    (`dialogue.rs:202`), FSM-like actions and panel UI (`:85-100`, `commands`, `Visibility`, `Text`). "Dialogue
    timer moves" means either moving the whole system or splitting it; see N4.

#### Non-blocking

- **N1. Finding 6 is subtly wrong:** the first `app.update()` runs **0 ticks**.
  `ManualDuration` → `Real::update_with_duration` → `update_with_instant`, and the first call returns early with
  zero delta (`bevy_time real.rs:88-105`). Most tests do a warm-up `update()` (for example
  `ui_tests.rs:58`), but any test that writes a message or `Interaction` before the very first update and
  asserts after one update breaks once the pipeline lives in `FixedUpdate`. Document this in `support::tick`.
  Also: `corpse_loot_interact_tests.rs:386` uses 250 ms steps (16 ticks per update), not 1/64. Only 4 files
  actually insert an override; `local_coop_tests.rs:6695` is a comment. Prefer Bevy's built-in
  `TimeUpdateStrategy::FixedTimesteps(n)` (`bevy_time lib.rs:116-118`) over a home-made `tick(n)` for N-tick
  tests, and `ManualDuration(1/144)` for 0-tick cadence.
- **N2.** Test blast radius: "single digits to low tens" is optimistic for a different reason than the one
  given. `spawn_scene_v2` stays in `Update`, so `SceneEvent::Ready` reactions (`on_enter`, scene-ready
  bindings) move from same-update to next-update. Every "load scene, update until Ready, assert reaction"
  helper gains one update. Audit the `scene_lifecycle`, `entity_logic` and `local_coop` helpers first. They are
  shared, so the change is cheap but widespread.
- **N3.** Approach 7 calls `stable_secs`/`stuck_secs` "wall-clock-fed". `gamepad_bind_system` already runs in
  `FixedUpdate` and accumulates `Time<Fixed>` dt (`runtime/input.rs:196-209`). The nondeterminism comes from
  the hardware connect/disconnect timing, not the clock. Reword it.
- **N4.** `dialogue_tick_system` should be split into sim (`ActiveDialogue` advance, conditions, actions) and an
  `Update` projection (panel `Visibility`/`Text`/choice-button spawn). Otherwise panel updates are quantised to
  ticks, and UI entity spawns happen 0-16x per frame. The same "sim state + render projection" rule should be
  written down once in `src/CLAUDE.md`, because `inventory_ui`/`container_ui` already follow it.
- **N5.** Click capture in `BeforeFixedMainLoop` reads camera and selectable `GlobalTransform` from the previous
  frame's `PostUpdate`. That matches the image the user actually clicked, so it is arguably *more* correct than
  today (see the update-side GlobalTransform staleness memo). State it, so nobody adds a
  `fresh_global_transform` call there by reflex.
- **N6.** The WASM catch-up cost is now (physics + interpreters + executor + stat loops) x up to 16 ticks on one
  thread. Add a Phase 3 measurement at a forced 250 ms stall on `local_coop_demo` (4 players, 4 bars), not only
  at a steady 30 fps. Recommend documenting that `max_frame_delta_secs` around 0.1 is the advised setting for
  heavy web projects.
- **N7.** `SimTick` increments in `Begin`. Under B1 it must also be gated by the latched `SimActive`, and the
  harness hash should be taken at end of tick (after `Writeback`) so it covers the physics result. Say where
  the Phase 4 hook sits.
- **N8.** `Time<Fixed>::elapsed` diverges from `Time<Virtual>::elapsed` by the overstep. No moved system
  reads `elapsed` today (checked: executor, interpreters, stats, action bar, targeting). Add a one-line rule to
  `src/CLAUDE.md`: fixed-tick systems use `delta_secs()` or `SimTick`, never `elapsed`.
- **N9.** Approach 4's latency claim is fine. Also check the claim that `Interaction` and `ButtonInput` are fed in
  `PreUpdate` and seen before the capture system: `ui_focus_system` is in `PreUpdate`, which runs before
  `RunFixedMainLoop` (`bevy_app main_schedule.rs:218`). Correct.
- **N10.** Completeness:
  - `Planned at`, the phases table, the tasks, the playtest list and Given/When/Then acceptance criteria are
    all present.
  - Missing: a per-phase "what ships and what is temporarily wrong" line (B5).
  - Missing: an acceptance criterion for B1's pause-timer, Resume-step and collision cases.
  - The Phase 3 reviews list belongs in the workflow, not the tasks.
- **N11.** Approach 3 rejects the `InputQueue` of typed commands as "duplicating Messages". Note that
  `SimInput` already has to become per-player and keyed by `SpawnId` (B4), which makes it most of the way to
  that command stream. Name the convergence target, so Phase 4 does not grow a third mechanism.

#### Open questions for Frank

1. **Pause mechanism.** Recommend: keep the `Time<Virtual>` freeze (it is the only cheap animation-freeze
   lever, because Bevy advances `AnimationPlayer` off `Time<Virtual>`), plus a control tick. Spike
   `world.run_schedule(FixedMain)`, which gives dt = 0 for free, before `accumulate_overstep`. Gate with a
   **whitelist** latched in `Begin`, owned by real_pause's single clock owner (B1). Reject `SimPaused` without
   the Virtual freeze for v1. Revisit it at Beta 0.6, when networked pause cannot freeze a shared clock
   anyway (real_pause scope decision 4). Ship real_pause either **before** Phase 3 with the control tick in
   mind, or inside Phase 3. Do not ship it with an `Update`-interpreter assumption that Phase 3 then breaks.
2. **≤ 15.6 ms click-to-effect.** Accept. It equals the movement granularity players already feel, it is 0 at
   ≤ 64 fps, and splitting the interpreter would break D3's single-order guarantee.
3. **Spawn throttle.** Neither option as posed (B2). Recommend a per-tick cap on logical spawns, with the value
   2 to keep today's wave timing at 60 fps, plus a per-frame cap on render attachment. Decide it in Phase 3,
   not Phase 4.
4. **Phase order.** Recommend D3, then Phase 1, then Phase 2, then D1 (independent, can go earlier), then Phase
   3 (needs real_pause decided and B1–B3 resolved), then D4, then Phase 4, then D5 plus the harness. Phase 1
   must not go before D3 (B5). D1/D2 are not ordering prerequisites for Phase 3's schedule work, only for the
   harness passing.

### UX-gamedesigner review

## UX plan-review: gameplay_fixed_tick_pipeline.md (designer / player view)

Reviewed: `planning/features/gameplay_fixed_tick_pipeline.md` (Draft, planned at `a91a080`), checked against
`docs/20`, `docs/30`, `docs/40`, `docs/STATUS.md`, `docs/10`, `docs/50`, `planning/features/real_pause.md`,
`planning/features/spawn_wave_encounter.md`, and every timer value authored in `assets/projects/*`.

#### Verdict

**Needs more design work** (narrow). The engineering design is sound and the designer-facing promise
("you author nothing differently, seconds stay seconds") holds. Nothing shipped depends on sub-tick
timing. Two things are missing before coding: (1) the pause decision has no designer-visible semantics
attached, and (2) the docs tasks aren't split by phase and leave out surfaces that each phase makes
wrong. Both are plan edits, not redesigns.

##### Authored time values: what a designer actually feels

- Every authored timer stays in seconds: `delay_secs`, `cooldown_secs`, `duration_secs` (modifiers,
  camera shake, despawn fade), `regen_rate` (per second), `advance_delay_secs`. They keep counting
  seconds with a constant `dt = 1/64`. Correct, and the right call. Rejecting integer tick deadlines is
  also good for designers.
- **Effective resolution.** A timer fires on the first tick at or after its deadline. In practice it can
  be up to 15.6 ms late. Today it is quantised to the *frame* (16.7 ms at 60 fps, 33 ms at 30 fps), so
  on 30 fps machines the timing gets more precise, not less. No designer will perceive this. It still
  needs one sentence in the docs (see Blocking 2).
- **Shortest shipped values:** 0.05 s (torch, star_shower and lightning_orb flicker loops in
  `particles_demo` / `effect_mayhem_demo`) and 0.18 s. These become 4 ticks (62.5 ms) and 12 ticks
  (187.5 ms), against roughly 3 and 11 frames today at 60 fps. Flicker cadence changes by a few ms and
  nobody will see it. The 0.2 s and 0.3 s cooldowns in `3rd_person_game_demo` (`main.scene.ron` :474,
  :489) become 13 and 20 ticks, also fine. **No shipped demo depends on sub-tick timing.**
- **What becomes more consistent:** chained events (EmitEvent -> handler -> EmitEvent...) cost one tick
  per hop at every frame rate. Today a hop costs one frame: 35 ms at 144 Hz, 167 ms at 30 fps. This
  real, designer-visible improvement should be stated in the docs: "each hop of an event chain takes
  1/64 s, on every machine."
- Stat bars and the cooldown sweep become render-rate *mirrors* of 64 Hz state. At 144 Hz a regen bar
  or cooldown radial moves in 64 Hz steps. That is imperceptible, so no interpolation is needed.

#### Blocking

1. **Pause (Q1) has no designer-facing semantics. Each mechanism silently changes what time-based
   authoring does inside a pause/menu state.** With the recommended "paused control tick", the
   `DelayedEvents`, `Stats`, despawn-timer and dialogue sets are gated by `SimActive`. Anything a
   designer arms *from* the pause menu therefore never fires until resume. That includes
   `EmitEventAfterDelay` (a "Saved!" toast hidden after 2 s, or a delayed `LoadScene` after a Quit
   fade), `SetDespawnTimer`, the despawn `fade_secs`, `CameraShake` duration, and dialogue
   `advance_delay_secs` if a dialogue is open. Nothing in the plan says so. The ActionBar's built-in
   `do_actions` are not state-gated (an inventory slot toggles while paused), so cooldowns started
   while paused also freeze. The plan must add one explicit table, "keeps running while paused /
   frozen while paused", covering: UI clicks, key bindings, FSM transitions and entry_actions,
   EmitEvent (immediate), EmitEventAfterDelay, cooldowns, despawn timers and fades, regen/modifiers,
   dialogue auto-advance, audio actions, camera shake, particles and animation. That table goes into
   the `real_pause.md` amendment *and* docs/30. Also add a validate warning or a doc callout:
   "`EmitEventAfterDelay` authored in a state whose entry pauses the game will not fire until resume."
   Without this, the first designer who builds an animated pause menu is stuck with a silent no-op.

2. **The docs tasks aren't per-phase and miss surfaces each phase makes wrong.** Each phase ships on its
   own, but docs work sits only in Phase 2 (`docs/40` input seam), Phase 3 (docs/30 heading rename) and
   Phase 4 (`docs/40` rewrite). Missing surfaces:
   - Phase 1:
     - `docs/30` ~880: "`tick_delayed_events_system` runs before the interpreter chain" now names the
       wrong schedule.
     - `docs/30` ~186: thresholds are "available to the rule interpreter on the next frame". After
       Phase 3, D-a makes them same-tick.
     - `docs/30` ~390: `ActionQueue` is "processed each frame".
     - `docs/20` ~4628: the `max_fixed_delta_secs` note distinguishes FixedUpdate from Update systems.
       After this plan, gameplay timers fall under the tick budget, and the slow-motion footgun now
       also slows cooldowns, delays and dialogue.
     - Add a shared "Timing resolution" note near the docs/20 timer fields: "timers resolve to 1/64 s
       ticks; a timer fires on the first tick at or after its deadline."
   - Phase 3:
     - the whole docs/30 "System ordering" block (~873-884): it says "the following `Update` tick" and
       "same frame";
     - the D3 same-frame section, which must state the *whole* model in designer words: UI first, then
       carry-over from the previous tick, then world/targeting/action bar/interact/delayed/stats, then
       scene;
     - `docs/30` ~203 "per tick/frame";
     - `docs/30` ~310 and ~335, `docs/50` ~54, `docs/10` ~37, `docs/40` ~104 and ~216-223 (Milestone A):
       mark "fixed tick gameplay loop" as done.
   - Phase 4:
     - `docs/30` ~49 and `docs/STATUS.md` ~91: both say "max 2/frame" for `Spawn`;
     - `planning/features/spawn_wave_encounter.md` ~283 ("takes 5 frames").
   - Also put `docs/dev/browser_tests.md` (DebugState) on the list if `SimTick` is exposed there (see
     Non-blocking 1).

#### Non-blocking

1. **The playtest checklist can't verify its own "same tick" claims.** The line "cooldown overlay and
   monster respawn land on the same tick on both rates" is unobservable for a human. Expose `SimTick`
   in `DebugState` (DOM, so `test_web.py` can read it) and in the diagnostics HUD, then compare logs.
   Separately, the respawn delay authored in `3rd_person_game_demo` is **30 s**, not 60 s
   (`enemy_*.behavior.ron`, `delay_secs: 30.0`; the corpse uses 600 s and 20 s). Needs verification
   against the plan's "60 s".
2. **Playtest checklist gaps** (add these):
   - Quick press-and-release clicks at a forced 30 fps and at 144 Hz on: action-bar slots, inventory and
     shop buttons, container Take/Take All, dialogue choice buttons, and the pause Resume button. This
     is exactly the class of bug in finding 5 that the capture move fixes, so it needs a human check.
   - `scene_key_bindings`/`global_key_bindings` keys and the gamepad Start button at 144 Hz (0-tick
     frames).
   - `stats_demo` / `primitive_world`: a buff expires on time (8-15 s modifiers), regen 5/15 per second
     looks smooth, and threshold events (e.g. low-health) still fire.
   - `particles_demo` / `effect_mayhem_demo`: short-delay flicker loops (0.05 s / 0.18 s) still loop,
     with no stall or burst.
   - `entity_logic_demo` `respawning_gem`: interact vs `gem.reappear`. This content is order-sensitive.
   - `3rd_person_game_demo` dialogue `npc_intro` auto-advance (3.0 s), checked at 30 fps and 144 Hz.
   - Tab away from the browser tab for 5 s and back. The 250 ms catch-up burst may fire several delays
     or cooldown expiries in one rendered frame, plus a spawn burst. It should feel like a hitch, not a
     glitch.
   - `primitive_world` death while the pause overlay is up. The `game_over` transition is
     `from: "playing"` only, and real_pause interacts with that.
3. **"`test_web.py` baselines unchanged" is optimistic** for particle/campfire scenes (`particles_demo`,
   `effect_mayhem_demo`, `primitive_world`). When the first delayed `SpawnEffect` fires can shift by up
   to a tick, and so can the scene-ready spawn timing. Expect particle-phase pixel diffs and say up
   front that re-baselining those projects is allowed, so nobody chases a non-bug.
4. **Expected-divergence list (docs/40) needs a one-line designer translation.** Designers and players
   care that "the same inputs give the same gameplay events, but visuals (particles, animation pose)
   and load timing can differ per machine". Don't make them read the harness hash field list.
5. **Spawn-timing wording for designers.** After Phase 4 the cap is "64 spawns per second" on every
   machine. Today it is 2 per frame, which works out to 60/s at 30 fps and about 290/s at 144 Hz. State
   it as a rate in docs/30 and STATUS, not as "per tick" jargon.
6. **Event-chain hop latency.** Add the "1/64 s per hop, on every machine" sentence to the docs/30 order
   section (see the authored-values notes above). It's a selling point and it removes a frame-rate
   dependency designers could hit today.

#### Open questions for Frank (designer-centric recommendations)

1. **Pause mechanism: recommend the paused control tick (keep the `Time<Virtual>` freeze).** For
   designers, the big win of real pause is that *everything* visibly freezes for free: animation,
   particles, physics, timers. The `SimPaused`-only gate would make designers (or the engine) pause
   animation, particles and audio separately, and a half-frozen world is worse UX than either extreme.
   Condition: ship the "runs while paused / frozen while paused" table from Blocking 1, and decide
   explicitly whether `EmitEventAfterDelay` armed while paused waits for resume. Recommendation: it
   waits. That is consistent and simple, and it gets a doc callout. A real-time delay variant can be a
   later, separate item if a menu ever needs one. If the spike fails and the fallback is chosen, the
   same table is needed and the fallback must still freeze animation and particles, or the pause feels
   broken to players.
2. **UI-action latency: accept the ≤ 15.6 ms granularity.** It sits below the one-frame latency players
   already absorb, at 30 fps it is effectively zero, and it buys a single, documentable event order. The
   alternative (splitting the interpreter) would bring back the "it depends which stream" order
   confusion D3 exists to remove. Measure the worst case at 144 Hz in the playtest, but treat it as a
   check, not a gate.
3. **Spawn throttle: recommend `SPAWNS_PER_TICK = 1`.** A 20-enemy wave over 0.31 s instead of 0.17 s
   reads as a natural stagger, and enemies popping in one after another is arguably better feel. The
   alternative, catch-up bursts of up to 32 spawns in a single frame, recreates the WebGPU compile
   hitch, which players *do* notice. `3rd_person_game_demo` spawns at most a handful at once (6 monster
   slots), so nothing visible changes there. Update `spawn_wave_encounter.md` ~283 to talk in ticks and
   seconds. If waves later need instant appearance, the designer lever belongs in that feature (for
   example a pre-warm/preload step), not a global cap.
4. **Phase order vs D3: recommend Phase 1 first and independently**, with its own docs edits
   (Blocking 2). Phase 1 has no designer-visible behaviour change, makes timers frame-rate-independent
   right away, and doesn't touch the event-order wording D3 is about to publish. Phase 3 must land after
   D3 *and* after real_pause's decision, so the docs/30 order section is rewritten once ("same-tick")
   rather than twice. Don't publish D3's "same-frame" wording if Phase 3 is close. Write it in
   tick-neutral language ("in one step of the game") so it survives Phase 3 unchanged.

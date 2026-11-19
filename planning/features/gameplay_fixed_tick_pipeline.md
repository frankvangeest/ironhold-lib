# Feature: Gameplay-Logic Pipeline and Timers on the Fixed Tick

_Status: Draft, revision 2 (2026-11-19). Rewritten against the shipped D3 sets and real pause; folds in every blocking finding of the 2026-10-02 plan-review (resolution table at the end of this head). Needs plan-review round 2 and Frank's answers to Q1-Q5 (recommendations recorded) before it is Ready._
_Planned at: `ef169e7` (2026-11-19); revision 1 was planned at `a91a080` (2026-10-02)_

Backlog item: Beta 0.5 > **"Move the gameplay-logic pipeline and its timers onto the fixed tick"**; first step of the **Multiplayer critical path** in `planning/backlog.md`. Builds on `deterministic_fixed_timestep.md` v1 (Done), **D1, D2, D3** (all shipped: `runtime/schedule.rs`, `tests/schedule_order_tests.rs`, `tests/determinism_lint.rs`) and **real pause** (`planning/features/done/real_pause.md`, shipped 2026-10-14: `SimClock`). Siblings: D4 (`SpawnId` tie-breaks), D5 (two-`App` smoke test), "Cross-platform determinism harness" (v2). Source audit: `planning/investigations/hashmap_iteration_order_audit.md` (findings A, B, F, I).

## What changed since revision 1 (why this is a rewrite of the Approach)
- **D3 shipped.** The sets exist (`GameplaySet`, `EmitSet`, `GameStreamSet`) but all live in `Update`. Phase 1 no longer has an "if D3 has not shipped" branch (blocker B5 is gone).
- **Real pause shipped** with the opposite assumption to this plan's: the interpreters and executor run in `Update`, so Resume works while `Time<Virtual>` is frozen. `SimClock` (holds `Menu`/`Static`/`FocusLoss`/`Session`) is the single clock owner; `apply_sim_clock_system` runs right after the executor; `keep_messages_flowing_while_held` (in `Last`) exists only because a held clock runs zero fixed ticks. The pause design below is built on those, not on a new flag.
- **Many systems joined the sets after revision 1** (item system S2-S6, container paging planned, backend lobbies): `inventory_slot_click_system`, `bar_assign_click_system`, `default_panel_actions_system`, `slot_menu_sync_system`, `action_bar_item_slot_system`, `apply_bar_assignments_system`, `bar_assign_sync_system`, `backend_glue_system` (wall clock!), `cursor_capture_system`, `aim_target_system`, `action_bar_visual_system`. The layout table in Approach 5 is built from the live manifest (`schedule_order_tests::the_documented_systems_are_in_the_documented_sets`), not from memory.

## Phases
| Phase | Backlog item | Size | Status |
|---|---|---|---|
| 1 | `SimTick` + `FIXED_DT`; `tick_delayed_events`, `cooldown_tick`, `despawn_timer`, stat modifier/regen/effective onto `FixedUpdate`, after `npc_behavior` and before `motion` | S-M | Queued, can start now |
| 2 | `SimInput` edge latch, only for readers **already** in `FixedUpdate` (`input_translator`) | M | Queued |
| 3 | **Capture layer:** every wall-clock, window, camera and `Interaction` system moves to `RunFixedMainLoop`/`BeforeFixedMainLoop`; the per-schedule dead-edge test; the interpreters stay in `Update` | M-L | Queued |
| 4 | **Sim move:** `Interpret`/`Execute`/`PostExecute` and the sim-only sets into `FixedUpdate`; the whitelist control tick for pause; the `Execute` split; dialogue split; spawn cap | L | Queued, needs Q1 and Q3 |
| 5 | Leftovers: behavior activation tick-gating, state-hash hook, expected-divergence list, docs rewrite | M | Queued |

Each phase merges and ships on its own. Phases 1-3 are valuable even if 4 never lands (frame-rate-independent timers, no lost presses, no dead edges). **Phase 3 is new:** revision 1 moved capture and the sim in one L step; capture alone is safe to land first because the interpreters still run in the same frame, after `FixedMain`.

## What
Everything that decides what *happens* in a game becomes a function of the fixed tick, not of the render frame: delayed events, cooldowns, despawn timers, stat regen and modifier expiry, dialogue auto-advance, and finally the Message -> Interpreter -> Action -> Executor chain itself. A designer authors nothing differently (`delay_secs`, `cooldown`, `duration` stay in seconds). Observable result: the same inputs produce the same events on the same tick at 30, 60 and 144 fps, which is the precondition for replay and for lockstep (`networking_multiplayer.md` Form 1). Purely cosmetic systems stay on the render clock. Designer-visible gains: an event chain costs 1/64 s per hop on every machine (today one frame per hop), and timers fire on the first tick at or after their deadline (up to 15.6 ms late; on a 30 fps machine this is more precise than today).

## Why
Fixed-timestep v1 moved physics-adjacent systems only (the fixed chain is `gamepad_bind -> input_translator -> player_movement -> view_box_clamp -> collectible -> trigger_zone -> npc_behavior -> motion -> mark_dirty_trees`, `.before(PhysicsSet::SyncBackend)`, `lib.rs:416-428`). Everything that changes game state is still `Update`, driven by `Time<Virtual>` deltas. Two machines at 60 and 144 fps expire a 2.5 s cooldown on different ticks relative to physics, and a 144 fps machine runs the interpreter 2.25x more often than it steps physics. Bit-identical Rapier output does not help: the *inputs* to the next tick differ. The harness (v2) would report divergence at the first delayed event or cooldown regardless of Rapier. This plan makes the harness measure the whole simulation.

## Findings (verified against code and vendored Bevy 0.18.0; unchanged from revision 1 unless marked)
1. **`Res<Time>` inside `FixedUpdate` is already `Time<Fixed>`** (`bevy_time-0.18.0/src/fixed.rs:243-255`): moving a timer system there gives `delta_secs() == 1/64` with no body change; constant `dt` makes f32 subtraction bit-deterministic.
2. **Frame order is `First, PreUpdate, StateTransition, RunFixedMainLoop (FixedMain x N), Update, PostUpdate`.** `FixedUpdate` runs *before* `Update`. `ButtonInput`/`Interaction` are fed in `PreUpdate`, so anything in `RunFixedMainLoop` or `FixedUpdate` sees this frame's edges; an `Update`-written message is first seen by the *next* frame's tick.
3. **Message double-buffering is fixed-loop-aware** (`signal_message_update_system` in `FixedPostUpdate`): a message written in `Update`/`BeforeFixedMainLoop` survives 0-tick frames and is delivered exactly once across 2-tick frames. Hazard (architect B1): a reader that does not run while two flips happen loses messages, so no message reader may sit behind a gate (Approach 6).
4. **Polled state is not protected like messages.** `ButtonInput::just_pressed` is cleared each `PreUpdate`; a `FixedUpdate` reader (`runtime/input.rs:373-387` jump/run) loses a press on a 0-tick frame (about 56 % of frames at 144 Hz) and sees it twice on a 2-tick frame (about 4 per second at 60 Hz). Code-reading only, **not yet reproduced**; Phase 2's first task is the failing test.
5. **`Changed<Interaction>` is a partial latch**: a fixed-rate system spanning several frames collapses press+release to the final `None`. Click capture must stay a once-per-frame system (Phase 3).
6. **Tests pin one tick per `app.update()`** (`tests/support/mod.rs:79`, `ManualDuration(1/64)`), **except the first `update()`, which runs 0 ticks** (`bevy_time real.rs:88-105`; architect N1). `corpse_loot_interact_tests` steps 250 ms (16 ticks per update).
7. **Pause (revised):** a held `SimClock` pauses `Time<Virtual>`, which runs **zero** fixed ticks. With the interpreters in `FixedUpdate` a Resume button would never be processed; Approach 6 is the design for that.
8. **`Time<Virtual>::max_delta`** bounds catch-up (default 250 ms = 16 ticks; project override `max_frame_delta_secs`, min 2 ticks).
9. **New:** several sim-relevant states are written by *UI-mode* systems and read by the executor side: `LoadedInventoryUi.menu_action`, `assign_handoff`, `assign_mode` (S4/S6) and, planned, the container handoff (`container_panel_paging.md`). Together with the clicked `UiEvent`, these handoffs **are the input** that `default_panel_actions_system` turns into actions. They must be captured with their event (Approach 3), or a replay of the events alone would sell a different quantity.
10. **New:** `backend_glue_system` reads `Time<Real>` and talks to an adapter (heartbeats, timeouts, online replies). Online replies are external input, not simulation: it belongs to the capture layer, and a replay must record the reply events it writes.

## Approach

### 1. What ticks on the fixed clock (decision)
- **Fixed tick (sim):** delayed events, cooldowns, despawn timers, stat modifier/regen/effective/threshold, the interpreters, `flush_pending_intent`, `default_panel_actions`, the executor, `apply_sim_clock`, the *logical* half of the spawn drain, `action_bar_input`, `interactable`, tab-targeting and target auto-clear, `resolve_pending_behaviors`, `npc_hit_relay`, the sim half of dialogue, `apply_bar_assignments_system` (it writes `ActionSlotItem`, which `action_bar_input` reads), `update_player_speed`.
- **Capture layer (once per frame, before the tick, also while paused):** see the table in Approach 5.
- **Render rate (projection, unchanged):** `damage_popup`, `fading_light`/`fading_decal`, camera chain, animation, nameplates/world labels, stat bars, radar, `inventory_ui`, `container_ui`, `slot_menu_sync`, `bar_assign_sync`, `action_bar_visual`, `action_bar_item_slot`, particle simulation and mesh rebuild, `drain_dynamic_stat_ui`, `spawn_decal`, `audio_volume_var`, audio. They only mirror sim state and lose their `.after(action_executor_system)` edges (cross-schedule; at worst they lag one frame).
- **Scene/asset I/O stays `Update`** (`check_project_loaded`, `spawn_scene_v2`, preloads): asset-load-driven, cannot be tick-exact (audit finding B). Its `SceneEvent`s enter the tick stream via finding 3 (reaction one frame later than today; N2 below). The executor's deferred `SceneHandleV2` commands are applied at the end of `FixedMain` before `Update` runs `spawn_scene_v2` (every schedule run applies its final deferred commands, architect check (e)); keep the confirming test.
- **Rule (writes into `src/CLAUDE.md`):** *sim state* lives in resources and components written only in the fixed tick; *projections* read it in `Update` and never feed back. `inventory_ui`/`container_ui` already follow it. UI-mode state (menu open, assign mode, page) belongs to the capture layer, and its outputs (the `UiEvent` plus the handoff payload) are what the sim consumes.

### 2. Clock representation (decision, unchanged)
`FIXED_DT = 1.0 / FIXED_TICK_RATE`, and a `SimTick(u64)` resource advanced by the first system of each *active* tick. Timers keep counting seconds with the constant `dt`; `SimTick` is for stamping (latched inputs, logs, harness hashes, replay). Rejected: integer tick deadlines (changes authored semantics and every type for no determinism gain, `1/64` is exact in f32), `Time<Virtual>`/`Time<Real>` deltas in sim, `Time<Fixed>::elapsed()` as the counter. **Rule:** a fixed-tick system uses `delta_secs()` or `SimTick`, never `elapsed()` (architect N8; `Time<Fixed>::elapsed` diverges from `Time<Virtual>::elapsed` by the overstep).

### 3. Crossing the capture -> fixed boundary (decision, revised after B4)
Rule: everything that touches wall-clock, window, GPU, camera or `Interaction` is a **capture** system; capture systems translate and never read sim state.
1. *Messages* (`UiEvent`, `GameEvent`, `SceneEvent`): nothing to build (finding 3).
2. *Polled edges* (`ButtonInput`, gamepad, mouse): new `SimInput` resource, written by a capture system **once per frame** in `RunFixedMainLoopSystems::BeforeFixedMainLoop` that *unions* the frame's edges (a 0-tick frame accumulates, press+release survives). It is **cleared last in each tick**, so only the first tick of a multi-tick frame sees an edge; level state (held keys, stick axes) is sampled live. **Shape (B4):**
   - per-player, keyed by player `SpawnId` (never `Entity`): gamepad edges per pad resolved to the owner at capture time;
   - `TargetClick::{Select { player, target: SpawnId }, Clear { player }}` (the cursor-to-viewport owner decides `player`; "clicked empty space" is `Clear`);
   - slot clicks keyed `(bar.id, slot_key)` (slot_key alone is not unique across bars);
   - **handoff payloads travel with their event** (finding 9): the menu quantity, the assign handoff and the container handoff are written in the same capture pass as the `UiEvent` they belong to, and consumed by `default_panel_actions_system`. They are part of the recorded input.
   - A `SimInput` read outside `FixedMain` panics in tests (debug assertion), so a reader that is still in `Update` fails loudly.
   - **Convergence target (N11):** `SimInput` plus messages *is* the command stream a replay or lockstep layer records; Phase 5 names it, it does not add a third mechanism.
3. *UI interaction edges* (`Changed<Interaction>`, slot clicks, the `Yes` debounce on `Time<Real>`): `button_system`, `icon_button_click_system`, `inventory_slot_click_system`, `bar_assign_click_system`, `global_input_system`, `unclaimed_gamepad_trigger_system` (and the planned `container_click_system`) move to `BeforeFixedMainLoop` in Phase 3 and write messages plus handoffs. They run after `ui_focus_system` (`PreUpdate`).
- Click-to-target needs camera and cursor: it stays capture (`click_select_system`, `aim_target_system`), writing `TargetClick`. It reads the previous frame's `GlobalTransform`, which is the image the player actually clicked (N5): never add `fresh_global_transform` there by reflex.
- Rejected: an `InputQueue` of typed commands now (see convergence target), reading `just_pressed` in `FixedUpdate` (finding 4), waiting a frame (latency).

### 4. Latency and 0-/2-tick semantics (unchanged)
Capture runs immediately before `FixedMain` in the same frame: no extra frame; worst-case added delay is the wait for the next tick, <= 1/64 s (mean 7.8 ms at 144 Hz, about 0 at 30 fps). 0 ticks in a frame: nothing consumed, nothing lost (`ActionQueue`, `PendingIntentActions`, `DelayedEventQueue` persist; messages and edges accumulate). N ticks: each message read once, each edge seen by the first tick only, each tick fully drains `ActionQueue`. Event latency inside the tick shrinks from one frame to one tick per hop.

### 5. Tick layout and per-schedule table (B2, B3)
The D3 sets are configured in `FixedUpdate` as well (`GameplaySchedulePlugin` keeps its **exact registration spot**; WASM ties break by insertion order). **A set's configuration is per schedule**: a set that is empty in a schedule silently turns every `.before/.after(system)` against it into a no-op, so the dead edges below are inventoried and a test guards them (Phase 3).

| Schedule / slot | Systems (live manifest at `ef169e7`) |
|---|---|
| **Update** (unchanged for ever) | `check_project_loaded`, `spawn_scene_v2`, `preload_audio`, `preload_decals`, camera chain, animation, nameplates/world labels, particle sim and mesh rebuild, `damage_popup`, fading, `clear_pool_on_scene_unload`, audio |
| **Update, render projections of sim state** (Phase 4: lose `GameplaySet` membership) | `inventory_ui`, `container_ui`, `slot_menu_sync`, `bar_assign_sync`, `action_bar_visual`, `action_bar_item_slot`, `audio_volume_var`, `drain_dynamic_stat_ui`, `drain_particle_effects`, `simulate_pool`, `rebuild_pool_meshes`, `spawn_decal`, dialogue panel projection |
| **BeforeFixedMainLoop** (capture; also runs while held) | `global_input`, `unclaimed_gamepad_trigger`, `button`, `icon_button_click`, `inventory_slot_click`, `bar_assign_click`, (`container_click`), `click_select`, `aim_target`, `cursor_capture`, `load_backend_config`, `backend_glue`, `audio_state`, the `SimInput` capture |
| **FixedUpdate, always (the control-tick whitelist)** | Begin: latch `SimActive`, drain `CollisionEvent` into a persistent queue, advance `SimTick` (only if active) -> `Interpret` (`fsm`, `entity_fsm`) -> `Execute` (`flush_pending_intent`, `default_panel_actions`, `action_executor`, `apply_sim_clock`, `stat_effective_value` 2nd instance, `stat_threshold`, `drain_spawn_queue` logical half) -> `PostExecute` (`npc_hit_relay`, `apply_bar_assignments`) -> end: clear `SimInput` edges |
| **FixedUpdate, sim-only** (`run_if SimActive` on the set) | movement chain (`gamepad_bind -> input_translator -> player_movement -> view_box_clamp`); `GameStreamSet::World` = `collectible -> trigger_zone -> npc_behavior` (read the persistent collision queue, never the message); `Targeting` (`tab_targeting`, `target_auto_clear`), `ActionBar` (`cooldown_tick`, `action_bar_input`), `Interact`, `DelayedEvents`, `Stats`; `DirectActions` (`resolve_pending_behaviors`, `despawn_timer`); the dialogue sim half; `motion`, `mark_dirty_trees`, Rapier `SyncBackend/StepSimulation/Writeback` |

One tick, in order: Begin -> movement -> `Emit` (`Targeting -> ActionBar -> Interact -> World -> DelayedEvents -> Stats`, then `DirectActions`, `Dialogue`) -> `Interpret` -> `Execute` -> `PostExecute` -> `motion -> mark_dirty_trees` -> Rapier (pipeline **before** `SyncBackend`, so teleports and spawn effects reach Rapier the same tick; `CollisionEvent`s still come from the previous tick's step) -> clear edges. D3's accepted principle "input events, then delayed events, then stat events" is preserved because `World` events precede `DelayedEvents` precede `Stats` in the stream.

**Dead edges to rewrite** (silently no-op once members move schedule): `dialogue_tick.after(button_system)`; `audio_state_system.before(fsm_interpreter_system)` and D3's `.before(GameplaySet::Interpret)` (it exists so `mute_on_start` lands before `PlayMusicLoop`; moving it to `BeforeFixedMainLoop` keeps that true); `click_select/tab/auto_clear.before(action_bar_input_system)` (`targeting.rs:65`) if only one side moves; `inventory_ui`/`container_ui.after(action_executor_system)`; `npc_hit_relay.after(action_executor_system)`; D3's `aim_target`/`slot_menu_sync`-style `.after(...)` edges. **Phase 3 adds `schedule_order_tests::every_ordering_target_resolves_in_its_schedule`**: for `Update`, `FixedUpdate` and `RunFixedMainLoop`, every system-type ordering target resolves to at least one system *in that schedule*. D3's structure and source-scan tests become multi-schedule (the "`FixedUpdate` too" conflict check already exists).

### 6. Pause and `Time<Virtual>` (decision, revised after B1; Q1)
Keep real pause's design: `SimClock` is the single owner; `apply_sim_clock_system` is the only caller of `Time<Virtual>::pause()`; the freeze still stops animation, particles, physics and timers for free (Bevy advances `AnimationPlayer` off `Time<Virtual>`). Moving the interpreters into `FixedUpdate` would starve them while held, so:
- **One control tick per rendered frame while held**, produced by an exclusive `BeforeFixedMainLoop` system that calls `world.run_schedule(FixedMain)` directly when `!SimClock::is_running()`. (The normal fixed loop then gets a zero delta and runs no ticks.) Generic `Time` is `Time<Virtual>` there (paused, delta 0), so **every dt consumer stops by itself as a second line of defence**, and `Time<Fixed>::elapsed` stays untouched. Spike first (Phase 4, task 1); fallback is `Time::<Fixed>::accumulate_overstep(period)`; rejected for v1: a `SimPaused` flag without the `Time<Virtual>` freeze (animation, particles and audio would need separate pause levers; revisit at Beta 0.6, when a networked pause cannot freeze a shared clock anyway).
- **A whitelist, not a blacklist:** only the "always" row of the table runs in a control tick. Everything else sits under the `SimActive` set condition (evaluating a set condition once at the first member's turn is exactly the wanted semantics here; this amends `schedule.rs`' "run conditions stay on their systems", which was about per-system conditions).
- **`SimActive` is latched once per tick in `Begin`** from `SimClock::is_running()` and read through `resource_equals`, never a live `is_paused()` read. So: a Resume executed mid-tick runs **no physics step** that tick (the latch was false; `SimTick` did not advance), and a pause executed in tick k of an N-tick frame turns ticks k+1..N into control ticks. The gate is per tick, not per frame.
- **No gated message reader (finding 3):** `collectible` and `trigger_zone` read a persistent `CollisionEventQueue` filled by an ungated Begin system (the `NpcHitQueue` pattern), so a collision on the last step before a pause is still processed after Resume. `keep_messages_flowing_while_held` stays in `Last` for the render-side buffers (harmless and cheap with control ticks).
- **What a designer sees while paused (UX B1; goes into the `real_pause` amendment and docs/30):**

  | Keeps running while paused | Frozen while paused |
  |---|---|
  | UI clicks and key bindings (Resume, Quit), FSM transitions and `entry_actions`, `EmitEvent` (immediate), audio actions, `LoadScene` | `EmitEventAfterDelay`, cooldowns, despawn timers and fades, regen and modifiers, dialogue auto-advance and choices, camera shake, physics, animation, particles |

  Decision (recommended, Q4): an `EmitEventAfterDelay` armed while paused **waits for resume**. `ironhold validate` gets a warning for an `EmitEventAfterDelay` reachable from a `game.paused` binding or a state entry that pauses ("will not fire until resume"). A real-time delay variant is a possible later item.

### 7. The `Execute` split and the spawn cap (B2; Q3)
D3's `Execute` mixed sim and render work. **Phase 4 splits it explicitly:** fixed (whitelist) `flush_pending_intent -> default_panel_actions -> action_executor -> apply_sim_clock -> stat_effective_value -> stat_threshold -> drain_spawn_queue (logical)`; `Update` (render rate, draining persistent queues) `drain_dynamic_stat_ui -> drain_particle_effects -> simulate_pool -> rebuild_pool_meshes -> spawn_decal`, keeping `fading_*` `.after` edges. Otherwise particles step at 64 Hz with no interpolation, are not simulated on 0-tick frames, and the mesh rebuild runs up to 16 times in a catch-up frame on single-threaded WASM.
**Spawn cap, decided in Phase 4 (not Phase 5):** a per-tick cap on *logical* spawns (registry entry, components, collider; recommended value 2, which keeps today's wave timing at 60 fps) so entity existence is tick-deterministic, plus a separate **per-frame** cap on render instantiation (SceneRoot/material attach), because the WebGPU compile stall is driven by render attachment. `SPAWNS_PER_TICK = 1` alone still allows 16 per frame after a stall.
**Dialogue (N4):** `dialogue_tick_system` is split in Phase 4 into the sim half (advance, conditions, actions, in `Dialogue`) and an `Update` projection (panel `Visibility`/`Text`/choice buttons). Phase 1 leaves it in `Update`.

### 8. What stays non-deterministic after this plan
Documented in `docs/dev/40_determinism_and_networking.md` with a one-line designer translation ("the same inputs give the same gameplay events; visuals and load timing can differ per machine"): rendering and animation pose, audio, particle simulation; the **asset-load completion tick** (`SceneEvent::Ready`, `BehaviorHandle`, dialogue assets; harness answer: hold tick 0 until Ready and stamp the tick, or record load-complete as an input); **online replies** (`backend_glue`, finding 10: recorded as inputs); camera-derived input (click-to-target); the gamepad debounce `stable_secs`/`stuck_secs` (the clock is already `Time<Fixed>`; the nondeterminism is hardware connect/disconnect timing, N3); `Entity` indices (never tie-break on them, D4). The harness hashes `{SimTick, LogicState, GameVariables, ActionQueue length, CooldownMap, DelayedEventQueue, LoadedStats, SpawnRegistry keys, rigid-body transforms}` **at end of tick** (after `Writeback`, N7) and reports the first diverging tick; Phase 5 adds the hook and exposes `SimTick` in `DebugState` and the diagnostics HUD so a human or `test_web.py` can compare logs.

### 9. WASM / performance
Per-tick cost: these systems already run once per frame with early-outs. At 60 Hz render the load is about unchanged, at 144 Hz it falls (64 runs/s vs 144), at 30 fps it doubles per frame (2 ticks). Catch-up bursts are bounded by `max_delta` (<= 16 ticks), and the burst is now (physics + interpreters + executor + stat loops) x 16 on one WASM thread: **Phase 4 measures a forced 250 ms stall on `local_coop_demo` (4 players, 4 bars)**, not only a steady 30 fps, and docs recommend `max_frame_delta_secs` around 0.1 for heavy web projects (N6). Watch items: `entity_fsm_interpreter_system`, the `stat_*` loops, `action_bar_input`, the spawn drain.

### 10. Tests: blast radius and migration
~707 tests, ~1018 `app.update()` calls. dt-dependent tests are unaffected (one tick per update). The break class is **same-`update()` chains** and **scene-ready reactions**: `SceneEvent::Ready` reactions move from same-update to next-update because `spawn_scene_v2` stays in `Update` (N2), so every "load scene, update until Ready, assert reaction" helper gains one update. Audit the `scene_lifecycle`, `entity_logic` and `local_coop` helpers first (shared, so the fix is cheap but widespread); expect tens of edits, not single digits. Use Bevy's `TimeUpdateStrategy::FixedTimesteps(n)` for N-tick tests and `ManualDuration(1/144)` for 0-tick cadence; document the first-`update()`-is-0-ticks rule in `support`. New tests: 0-tick, 2-tick, 4-tick-burst and 144 Hz cadences for each of press, click, delayed event, cooldown, `ActionQueue` drain; a two-`App` test at 30 and 144 Hz cadences comparing event logs per `SimTick` (this is D5 extended; coordinate, do not duplicate). Run per-file (disk rule); `cargo check -p ironhold_cli` each phase.

## Tasks
**Phase 1 (S-M): timers**
- [ ] `FIXED_DT`, `SimTick`, `advance_sim_tick` (derive from `FIXED_TICK_RATE`)
- [ ] Move to `FixedUpdate` with no body change: `tick_delayed_events`, `cooldown_tick`, `despawn_timer`, `stat_modifier -> stat_regen -> stat_effective_value`; chain them **after `npc_behavior`, before `motion`**, in the D3 stream order (`DelayedEvents` then `Stats`); an explicit edge against the existing `GameEvent` writers (B5)
- [ ] Update `schedule_order_tests` membership (the manifest names `GameStreamSet::*` in `Update`) and the `lib.rs` comments; `capabilities/CLAUDE.md` timer notes
- [ ] State the **interim input-vs-timer inversion** (delayed and stat events now precede same-frame `Update` input events until Phase 4) in docs/30 and cover it with one test
- [ ] Tests: delayed event / cooldown / modifier expire on the same `SimTick` at 30/60/144 Hz cadences; a pause freezes them (zero ticks)
- [ ] Docs (Phase 1 surfaces): docs/30 "tick_delayed_events runs before the interpreter chain" wording, docs/20 "Timing resolution" note near the timer fields and the `max_fixed_delta_secs` slow-motion footgun note
**Phase 2 (M): SimInput for fixed readers only (B4)**
- [ ] Reproduce finding 4 as failing tests first (lost jump on a 0-tick frame, doubled on a 2-tick frame)
- [ ] `SimInput` (shape in Approach 3), `BeforeFixedMainLoop` capture, end-of-tick clear, the read-outside-`FixedMain` assertion
- [ ] Migrate `input_translator` (and any other reader already in `FixedUpdate`); **no `Update` reader switches here**
- [ ] Docs: `docs/40` "input seam"
**Phase 3 (M-L): capture layer**
- [ ] Move the capture row of the table to `BeforeFixedMainLoop`; handoff payloads written with their events (finding 9); `backend_glue` and `audio_state` placement
- [ ] `schedule_order_tests::every_ordering_target_resolves_in_its_schedule`, and the dead-edge rewrites listed in Approach 5
- [ ] Verify the deferred `SceneHandleV2` ordering with a test
- [ ] Playtest the click classes (quick press-release at 30 fps and 144 Hz on every button family)
**Phase 4 (L): sim move and pause**
- [ ] Spike: `world.run_schedule(FixedMain)` control tick vs `accumulate_overstep`; confirm dt = 0 for every consumer and a Resume mid-tick runs no physics step; amend `real_pause.md` (Q1)
- [ ] Move D3's sets to `FixedUpdate`; add `GameStreamSet::World`; split the movement chain; `SimActive` latch and set conditions; `CollisionEventQueue`
- [ ] The `Execute` split, the dialogue split, the spawn cap (Approach 7); switch the `Update` readers of `SimInput` in the same commit as their move
- [ ] Update `schedule_order_tests` (graph in `FixedUpdate`), `tests/support`, the scene-ready helpers; the designer table in docs/30 and the `validate` warning for `EmitEventAfterDelay` under a pausing state
- [ ] Docs: docs/30 "Same-frame event order" -> same-step wording written in tick-neutral language, "System ordering" block, `src/CLAUDE.md` pipeline section and the sim-state/projection rule, `docs/50`, `docs/10`, `docs/40` Milestone A marked done
- [ ] WASM: measure steady 30 fps and a forced 250 ms stall on `local_coop_demo`
**Phase 5 (M): leftovers**
- [ ] `resolve_pending_behaviors` activation tick-gating; click-select resolver; state-hash hook and the expected-divergence list; `SimTick` in `DebugState` and the HUD; `docs/40` rewrite; D4 coordination; spawn-rate wording in docs/30, `STATUS.md`, `spawn_wave_encounter.md` ("N spawns per second on every machine")

## Playtest checklist
- `3rd_person_game_demo` at native 144 Hz, 60 Hz and a forced 30 fps: jump, skill keys, mouse-click skill slots, tab-target, interact; mash jump at 144 Hz; quick press-and-release clicks on action-bar slots, inventory/shop buttons, container Take/Take All, dialogue choice buttons and the pause Resume button.
- Cooldown overlay and monster respawn (delay authored as **30 s** in `enemy_*.behavior.ron`, not 60) compared by `SimTick` stamps in `DebugState`.
- Pause: Resume click, Esc, Resume+Esc in one frame, pause during a delayed event (must not fire), `LoadScene` while paused, a `SetDespawnTimer`/`EmitEventAfterDelay` armed while paused waits for resume, a pickup collision on the last tick before pause is still processed after Resume, `primitive_world` death while the overlay is up.
- `scene_key_bindings`/`global_key_bindings` and the gamepad Start button at 144 Hz.
- `stats_demo`/`primitive_world`: buffs expire on time, regen looks smooth, threshold events fire. `particles_demo`/`effect_mayhem_demo`: 0.05 s and 0.18 s flicker loops keep looping. `entity_logic_demo` `respawning_gem`. Dialogue `npc_intro` auto-advance at 30 fps and 144 Hz.
- `local_coop_demo`: both players fire skills in one frame; hot-join with gamepad; 4-way split. Wave spawn timing (Phase 4). Chest/loot panel and the shop (S4-S6 click paths).
- Tab away from the browser for 5 s and back (250 ms catch-up burst: a hitch, not a glitch).
- `python test_web.py`: **expect particle-phase pixel diffs** on `particles_demo`, `effect_mayhem_demo`, `primitive_world` (first delayed `SpawnEffect` can shift by one tick); re-baselining those is allowed, nobody should chase it as a bug.

## Open questions (for Frank; both reviewers' recommendations recorded)
1. **Pause mechanism (blocks Phase 4).** Recommended: keep the `Time<Virtual>` freeze plus the whitelist control tick via `world.run_schedule(FixedMain)` (spike first, `accumulate_overstep` fallback). Alternative rejected for v1: a `SimPaused` gate without the freeze.
2. **Accept the <= 15.6 ms click-to-effect granularity** (UI-driven actions act on the next tick, not the next render frame). Recommended: accept (equal to movement granularity, 0 at <= 64 fps); the only alternative splits the interpreter and breaks the single-order guarantee.
3. **Spawn throttle.** Recommended: per-tick cap 2 on logical spawns plus a per-frame cap on render attachment, decided in Phase 4. (UX preferred 1 per tick; the architect showed that alone still allows 16 per frame after a stall.)
4. **`EmitEventAfterDelay` armed while paused waits for resume**, with a `validate` warning. Recommended: yes.
5. **Phase order.** D1-D3 and real pause are shipped, so: 1 -> 2 -> 3 -> 4 -> 5, with D4 before Phase 5 and D5 plus the harness after. Phase 1 can start immediately and is independent of Q1 and Q3.

## Acceptance criteria
- Given identical input streams stamped by `SimTick`, when run at 30, 60 and 144 fps cadences, then every timer-driven event (delayed, cooldown, despawn, regen, modifier expiry, dialogue advance) fires on the same `SimTick`, and the interpreter receives events in D3's order at the same tick.
- Given a key, click or gamepad press on a frame with 0 ticks, then it is applied by the next tick exactly once; on a frame with 2+ ticks, once, by the first tick.
- Given a held `SimClock`: no cooldown, despawn, delayed-event, regen, modifier, dialogue or camera-shake timer moves; `SimTick` is frozen; Resume/Quit/UI transitions still process in the control tick; a Resume mid-tick runs **no physics step** that tick; a collision from the tick before the pause is still processed after Resume.
- Given a slot click with a menu quantity (sell N), when the frame has several ticks or none, then the action carries the clicked quantity (the handoff travels with its event).
- Given any system ordering target in `Update`, `FixedUpdate` or `RunFixedMainLoop`, then it resolves to at least one system in that schedule (the dead-edge test).
- Cosmetic systems are unchanged; no new frame of UI latency beyond the <= 1 tick wait; 30 fps, 144 fps and a 250 ms stall stay within the agreed budget on `3rd_person_game_demo` and `local_coop_demo`.
- The existing suite passes after the documented mechanical migration; `ironhold_cli` checks pass; the expected-divergence list is documented in `docs/dev/40_determinism_and_networking.md`.

## Resolution of the 2026-10-02 plan-review blockers
| Finding | Where it is resolved |
|---|---|
| Architect B1 (pause gate: wrong set, Resume mid-tick, gated readers lose events) | Approach 6: whitelist, per-tick `SimActive` latch, `CollisionEventQueue`, single owner = `SimClock`, `run_schedule(FixedMain)` spike; acceptance criteria 3 |
| Architect B2 (`Execute` re-homing moves render work; spawn cap a phase early) | Approach 7 and the Phase 4 tasks |
| Architect B3 (per-schedule layout, dead edges) | Approach 5 table, dead-edge list, `every_ordering_target_resolves_in_its_schedule` (Phase 3) |
| Architect B4 (`SimInput` cleared before `Update` readers; shape) | Phase 2 limited to fixed readers; shape in Approach 3; readers switch in Phase 4 |
| Architect B5 (Phase 1 order inversion; D3 prerequisite) | D3 shipped; Phase 1 chained after `npc_behavior`; interim inversion stated and tested |
| UX B1 (pause semantics for designers) | Approach 6 table, Q4, `validate` warning |
| UX B2 (docs per phase) | Docs bullets under Phases 1, 3, 4, 5 |
| Non-blocking N1-N11, UX NB1-6 | N1 Approach 10; N2 Approach 10; N3, N7 Approach 8; N4 Approach 7; N5 Approach 3; N6 Approach 9; N8 Approach 2; N11 Approach 3; UX NB1-3, 5, 6 the playtest list, Phase 5 and the docs bullets |
| New in revision 2 | Findings 9 and 10 (handoff payloads are input; `backend_glue` is capture), the Phase 3 split |

## Plan-review round 1 (2026-10-02, historical: every blocker is resolved in the table above)

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

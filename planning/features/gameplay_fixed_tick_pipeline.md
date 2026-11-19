# Feature: Gameplay-Logic Pipeline and Timers on the Fixed Tick

_Status: Draft, revision 4 (2026-11-19); **Phase 1 Done 2026-11-19 (`038f79b`)**, Phases 2-5 Queued. Revision 2 rewrote the Approach against the shipped D3 sets and real pause; revision 3 folded in plan-review round 2; revision 4 folds in round 3 (whole plan). **Frank accepted all recommendations for Q1-Q5 on 2026-11-19. Round 3 verdict: Phases 1 and 2 are Ready (both reviewers; Phase 2 with one amendment, folded in below); Phases 3 and 4 were sent back for five architect and four UX blockers, all fixed in revision 4 and needing a short round 4 on the changed text; Phase 5 is planned after Phase 4.** Resolution tables for both review rounds are at the end of this head._
_Planned at: `ef169e7` (2026-11-19); revision 1 was planned at `a91a080` (2026-10-02)_

Backlog item: Beta 0.5 > **"Move the gameplay-logic pipeline and its timers onto the fixed tick"**; first step of the **Multiplayer critical path** in `planning/backlog.md`. Builds on `deterministic_fixed_timestep.md` v1 (Done), **D1, D2, D3** (all shipped: `runtime/schedule.rs`, `tests/schedule_order_tests.rs`, `tests/determinism_lint.rs`) and **real pause** (`planning/features/done/real_pause.md`, shipped 2026-10-14: `SimClock`). Siblings: D4 (`SpawnId` tie-breaks), D5 (two-`App` smoke test), "Cross-platform determinism harness" (v2). Source audit: `planning/investigations/hashmap_iteration_order_audit.md` (findings A, B, F, I).

## What changed since revision 1 (why this is a rewrite of the Approach)
- **D3 shipped.** The sets exist (`GameplaySet`, `EmitSet`, `GameStreamSet`) but all live in `Update`. Phase 1 no longer has an "if D3 has not shipped" branch (blocker B5 is gone).
- **Real pause shipped** with the opposite assumption to this plan's: the interpreters and executor run in `Update`, so Resume works while `Time<Virtual>` is frozen. `SimClock` (holds `Menu`/`Static`/`FocusLoss`/`Session`) is the single clock owner; `apply_sim_clock_system` runs right after the executor; `keep_messages_flowing_while_held` (in `Last`) exists only because a held clock runs zero fixed ticks. The pause design below is built on those, not on a new flag.
- **Many systems joined the sets after revision 1** (item system S2-S6, container paging planned, backend lobbies): `inventory_slot_click_system`, `bar_assign_click_system`, `default_panel_actions_system`, `slot_menu_sync_system`, `action_bar_item_slot_system`, `apply_bar_assignments_system`, `bar_assign_sync_system`, `backend_glue_system` (wall clock!), `cursor_capture_system`, `aim_target_system`, `action_bar_visual_system`. The layout table in Approach 5 is built from the live manifest (`schedule_order_tests::the_documented_systems_are_in_the_documented_sets`), not from memory.

## Phases
| Phase | Backlog item | Size | Status |
|---|---|---|---|
| 1 | `SimTick` + `FIXED_DT` (and `SimTick` in `DebugState`); `tick_delayed_events`, `cooldown_tick`, stat modifier/regen/effective onto `FixedUpdate`, after `npc_behavior` and before `motion`. **`despawn_timer` stays in `Update` until Phase 4** (moving it alone inverts D3's documented `ActionQueue` push order: activation, then despawn) | S-M | **Done 2026-11-19** (`038f79b`) |
| 2 | `SimInput` edge latch, only for readers **already** in `FixedUpdate` (`input_translator`); capture skips and clears edges while the clock is held | M | Queued; both reviewers: can start now |
| 3 | **Capture layer, UI only:** the scene stream (`check_project_loaded`, `spawn_scene_v2`) moves to `BeforeFixedMainLoop` **first**, then `button`, `icon_button_click`, `global_input`, `unclaimed_gamepad_trigger` and the slot-click systems, in their own sets; the multi-schedule set test and dead-edge test land **before any move**; the interpreters, `backend_glue` and `cursor_capture` stay in `Update` | M-L | Queued, needs round 4 |
| 4 | **Sim move:** `Interpret`/`Execute`/`PostExecute` and the sim-only sets into `FixedUpdate`; targeting capture (`click_select`, `aim_target` as `SimInput` samples); the per-event payload `Vec`s; the `backend_glue` split (`BackendInbound`) and the `cursor_capture` lock snapshot; `despawn_timer`; dialogue; the control tick; the `Execute` split; the spawn cap | L | Queued, needs round 4 |
| 5 | Leftovers: behavior activation tick-gating, click-select resolver, state-hash hook, expected-divergence list, docs rewrite | M | Queued; planned after Phase 4 |

Each phase merges and ships on its own. Phases 1-3 are valuable even if 4 never lands (frame-rate-independent timers, no lost presses, no dead edges). Phase 3 moves only systems whose consumers still run later in the same frame, so nothing can be lost; the systems whose output the *tick* consumes (targeting, payload handoffs) move with their consumer in Phase 4.

## What
Everything that decides what *happens* in a game becomes a function of the fixed tick, not of the render frame: delayed events, cooldowns, despawn timers, stat regen and modifier expiry, dialogue auto-advance, and finally the Message -> Interpreter -> Action -> Executor chain itself. A designer authors nothing differently (`delay_secs`, `cooldown`, `duration` stay in seconds). Observable result: the same inputs produce the same events on the same tick at 30, 60 and 144 fps, which is the precondition for replay and for lockstep (`networking_multiplayer.md` Form 1). Purely cosmetic systems stay on the render clock. Designer-visible gains: an event chain costs 1/64 s per hop on every machine (today one frame per hop: faster at 30 fps, about the same at 60, but **2.25x slower on a 144 Hz screen**, where a five-hop chain goes from about 35 ms to about 78 ms), and timers fire on the first tick at or after their deadline (up to 15.6 ms late; on a 30 fps machine this is more precise than today).

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
9. **New, handoff channels are input.** Several sim-relevant values are written by UI-mode systems and read by the executor side, next to the click event: `LoadedInventoryUi.menu_action` (S4, a **single `Option`**, `panel_defaults.rs:276/280`, taken at `:167`), `assign_handoff` (S6), the planned container handoff (`container_panel_paging.md`), and `PendingJoinGamepad` (`runtime/input.rs:105` resets it to `None` at the top of every run; `Action::JoinPlayer` takes it, `action_executor.rs:1669`; it is keyed by `Entity`, a local hardware identity). With capture before the tick and the consumer in `FixedUpdate`, a frame with no tick lets two clicks overwrite one `Option` (the second event then sells the wrong quantity or `all: true`; about 56 % of frames at 144 Hz) and lets the next capture pass wipe the join pad. A side slot cannot survive a 0-tick frame: the payload needs a mechanism (Approach 3).
10. **New, `backend_glue_system` is capture and writes sim state.** It reads `Time<Real>` and talks to an adapter (heartbeats, timeouts, online replies), so it is external input, not simulation. It also writes `GameVariables` directly (`backend.rs:629, 638-640`, including the lobby-expiry `drop_lobby`), and `apply_reply`, `lobby_tick` and `drop_lobby` need `&mut BackendHub` (in-flight and generation state, stale filtering, `backend.rs:690-704`) which the executor writes too (outbox, `dispatch_lobby_action` `:539`): a naive capture/sim split would share mutable hub state, and `load_backend_config_system` also writes `GameVariables` and events (`:1431-1437`). **Both backend systems stay in `Update` through Phase 3.** A held clock runs zero ticks until Phase 4's control tick, so a fixed-tick applier would drop pause-menu leaderboard and lobby replies (`keep_messages_flowing_while_held`'s own doc says it is safe only because the messages fixed consumers read are written in `FixedUpdate`), breaking docs/30:459-460. Phase 4 splits them: capture keeps the adapter, `started` and the real-clock timeouts and emits a new `BackendInbound` message (`BackendReply` already exists as the adapter's reply enum, `backend.rs:142`); the always-run row applies it (in-flight generations, `GameVariables`, events). `hub.outbox` is filled by the executor, so a request waits up to one frame, and on a frame with no tick a reply waits one more (state it).
12. **New, the cursor-capturing click must not also fire a mouse-button slot.** The action bar relies on `GameStreamSet::ActionBar` running before `::Interact` so that "the capture system runs after this one, so that click still sees `locked == false`" (`action_bar.rs:288-291`); with no runner lock report (native), `cursor_capture_system` sets `lock.locked = true` on the click itself (`cursor_capture.rs:135-137`). That order is implicit in set order, not a by-name edge, so the dead-edge test cannot see it. Moving `cursor_capture` ahead of the action bar would make the capturing click shoot.
11. **New, targeting capture writes sim state and is continuous.** `aim_target_system` (`targeting.rs:405-419`) raycasts Rapier and writes `PlayerTarget`, `CurrentTarget`, `GameVariables` and `GameEvent`; it is the *last* writer of `PlayerTarget` in the chain (`targeting.rs:62-80`). It is continuous aiming, not a click, so `TargetClick` cannot express it. Moved to capture it would become the *first* writer and tab-targeting/auto-clear would override it. Decided in Approach 3.

## Approach

### 1. What ticks on the fixed clock (decision)
- **Fixed tick (sim):** delayed events, cooldowns, despawn timers, stat modifier/regen/effective/threshold, the interpreters, `flush_pending_intent`, `default_panel_actions`, the executor, `apply_sim_clock`, the *logical* half of the spawn drain, `action_bar_input`, `interactable`, tab-targeting and target auto-clear, `resolve_pending_behaviors`, `npc_hit_relay`, the sim half of dialogue, `apply_bar_assignments_system` (it writes `ActionSlotItem`, which `action_bar_input` reads), `update_player_speed`.
- **Capture layer (once per frame, before the tick, also while paused):** see the table in Approach 5.
- **Render rate (projection, unchanged):** `damage_popup`, `fading_light`/`fading_decal`, camera chain, animation, nameplates/world labels, stat bars, radar, `inventory_ui`, `container_ui`, `slot_menu_sync`, `bar_assign_sync`, `action_bar_visual`, `action_bar_item_slot`, particle simulation and mesh rebuild, `drain_dynamic_stat_ui`, `spawn_decal`, `audio_volume_var`, audio. They only mirror sim state and lose their `.after(action_executor_system)` edges (cross-schedule; at worst they lag one frame).
- **The scene stream moves to `BeforeFixedMainLoop`, ahead of the capture sets (Phase 3):** `check_project_loaded` and `spawn_scene_v2` are asset-load-driven and cannot be tick-exact (audit finding B), but running them first keeps the shipped promise that input on a load frame sees the new scene's bindings and the released clock (docs/30:392-395, `schedule.rs` "Scene -> {Ui, Game}") **without any gate**, and in Phase 4 makes `SceneEvent::Ready` visible to the same frame's tick. Preloads stay in `Update`. **Phase 3's first spike verifies** that the system's asset-event and command timing is unchanged in that schedule. Its `SceneEvent`s enter the tick stream via finding 3. The executor's deferred `SceneHandleV2` commands are applied at the end of `FixedMain` before `Update` runs `spawn_scene_v2` (every schedule run applies its final deferred commands, architect check (e)); keep the confirming test.
- **Rule (writes into `src/CLAUDE.md`):** *sim state* lives in resources and components written only in the fixed tick; *projections* read it in `Update` and never feed back. `inventory_ui`/`container_ui` already follow it. UI-mode state (menu open, assign mode, page) belongs to the capture layer, and its outputs (the `UiEvent` plus the handoff payload) are what the sim consumes.

### 2. Clock representation (decision, unchanged)
`FIXED_DT = 1.0 / FIXED_TICK_RATE`, and a `SimTick(u64)` resource advanced by the first system of each *active* tick. Timers keep counting seconds with the constant `dt`; `SimTick` is for stamping (latched inputs, logs, harness hashes, replay). Rejected: integer tick deadlines (changes authored semantics and every type for no determinism gain, `1/64` is exact in f32), `Time<Virtual>`/`Time<Real>` deltas in sim, `Time<Fixed>::elapsed()` as the counter. **Rule:** a fixed-tick system uses `delta_secs()` or `SimTick`, never `elapsed()` (architect N8; `Time<Fixed>::elapsed` diverges from `Time<Virtual>::elapsed` by the overstep).

### 3. Crossing the capture -> fixed boundary (decision, revised after B4 and round 2)
Rule (corrected): everything that touches wall-clock, window, GPU, camera or `Interaction` is a **capture** system. A capture system may *read* sim state as of the last tick (`bar_assign_click` reads `assign_mode` and panel `Visibility`, `inventory_slot_click` reads `PlayerInventory`, `unclaimed_gamepad_trigger` reads `PendingEntitySpawns`/`BoundGamepad`), but **its effect on the sim enters only through recorded messages and payloads**, never by writing sim state. Where today a capture-like system writes sim state directly (`aim_target`, `backend_glue`), it is split: capture emits, a fixed-tick system applies.
1. *Messages* (`UiEvent`, `GameEvent`, `SceneEvent`): nothing to build (finding 3).
2. *Polled edges* (`ButtonInput`, gamepad, mouse): new `SimInput` resource, written **once per frame** in `RunFixedMainLoopSystems::BeforeFixedMainLoop`, *unioning* the frame's edges (a 0-tick frame accumulates, press+release survives), **cleared last in each tick** (only the first tick of a multi-tick frame sees an edge); level state (held keys, stick axes) is sampled live. **Shape:**
   - per player where the owner is known at capture; **gamepad edges are captured per pad and resolved to the owning player in the tick** (`gamepad_bind` runs in the tick after capture, so a pad that binds this tick must not lose its press); the recorded form is `(player SpawnId, edge)` stamped at tick start, never an `Entity`;
   - `TargetClick::{Select { player, target: SpawnId }, Clear { player }}` for clicks, and a **level sample** `Aim { player, target: Option<SpawnId> }` for continuous aiming, applied by a fixed-tick system **after** tab-targeting and auto-clear, which preserves today's "`aim_target` writes last" order (decided here, tested in Phase 4); the `TargetClick` applier takes today's first slot, at the head of `Targeting` before `tab_targeting` (click -> tab -> auto-clear -> aim, `targeting.rs:62-80`), gated by `SimActive` like today's `sim_running`;
   - slot clicks keyed `(bar.id, slot_key)`;
   - **while the clock is held (`!SimClock::is_running() || Time<Virtual>::is_paused()`) capture skips and clears edges**: before Phase 4 a held clock runs zero ticks, so unioned edges would fire on the first tick after Resume (today they are dropped); test it;
   - a read of `SimInput` outside `FixedMain` fails in tests: an `InFixedMain` flag is set in `FixedFirst` and cleared in `FixedLast` (Bevy exposes no "current schedule"); the `SimInput` clear and the flag clear are ordered explicitly, both in `FixedLast`.
   - **Convergence target (N11):** `SimInput` plus messages *is* the command stream a replay or lockstep layer records; Phase 5 names it, it does not add a third mechanism.
3. *Payload handoffs* (finding 9): **one rule.** A payload is **pushed in capture together with its event** into a `Vec` (`menu_action` becomes a `Vec`, `assign_handoff` already is one, the container handoff is one, and `PendingJoinGamepad` becomes `Vec<(trigger, Entity)>` read by `Action::JoinPlayer` by its triggering event; the pad `Entity` stays a local hardware identity and a replay records the join as an input and re-binds); it is **consumed by whoever handles that event in the tick**; and **every leftover is cleared at the end of the `Execute` of the first tick that reads those events** (that tick can be a control tick, and is always the first tick after capture). This covers a rule-owned event, a dropped event, an event no rule binds (a join with no `JoinPlayer` binding), a keyboard-triggered `JoinPlayer` taking no stale pad, and `LoadScene`, with no per-consumer popping; it is the pattern `default_panel_actions_system` already ships for `assign_handoff` ("anything left over is stale", `panel_defaults.rs:143-151`). Today's `unclaimed_gamepad_trigger_system` reset-every-run of `PendingJoinGamepad` (`input.rs:105`) is replaced by this rule. Phase 3 keeps today's slots (their consumer still runs later in the same frame); **Phase 4 converts them in the same commit that moves the consumer.** One exception to "UI-mode state belongs to capture" is stated: the *end* of `assign_mode`/`assign_end` is cleared by the sim at the end of the consuming event (`panel_defaults.rs:145-148`).
4. *UI interaction edges* (`Changed<Interaction>`, slot clicks, the `Yes` debounce on `Time<Real>`): `button_system`, `icon_button_click_system`, `inventory_slot_click_system`, `bar_assign_click_system`, `global_input_system`, `unclaimed_gamepad_trigger_system` (and the planned `container_click_system`) move to `BeforeFixedMainLoop` in Phase 3. They run after `ui_focus_system` (`PreUpdate`). **Each keeps its present pause gate** (see Approach 6): the Resume/Quit path runs while held, the bag-slot, action-bar, interact and targeting inputs stay no-ops.
5. *Backend* (finding 10): **stays in `Update` through Phase 3.** Phase 4 splits it (capture emits `BackendInbound`; the always-run row applies it), with the hub fields assigned explicitly: capture owns the adapter, `started` and the real-clock timeouts, the sim owns the in-flight generations and `GameVariables`.
- **Capture-layer ordering:** the capture systems get their own sets inside `RunFixedMainLoop::BeforeFixedMainLoop` (keeping today's UI chain order: `global_input -> unclaimed/claimed gamepad -> button -> icon_button -> slot clicks`), and the control-tick system (Approach 6) is ordered `.after` every capture set.
- **Scene-load frame (decided, no gate):** the scene stream runs first in `BeforeFixedMainLoop` (Approach 1), so on a Replace-load frame capture sees the new scene's bindings, buttons and released clock, as `runtime/schedule.rs` and docs/30:392-395 promise. The deferred `SceneHandleV2` still lands one frame later, as today. Test: a key and a bag click on the load frame use the new bindings and the released clock.
- **`cursor_capture` (finding 12):** stays in `Update`, after the action bar, through Phase 3. In Phase 4 the mouse-edge capture records `locked` **as it was before** `cursor_capture` ran into `SimInput`, and the action bar reads that snapshot. Test: first person, mouse-button slot, native (no runner report): the capturing click does not activate the slot.
- Click-to-target needs camera and cursor: it reads the previous frame's `GlobalTransform`, which is the image the player actually clicked (N5): never add `fresh_global_transform` there by reflex.
- Rejected: an `InputQueue` of typed commands now (see convergence target), reading `just_pressed` in `FixedUpdate` (finding 4), waiting a frame (latency).

### 4. Latency and 0-/2-tick semantics (unchanged)
Capture runs immediately before `FixedMain` in the same frame: no extra frame; worst-case added delay is the wait for the next tick, <= 1/64 s (mean 7.8 ms at 144 Hz, about 0 at 30 fps). 0 ticks in a frame: nothing consumed, nothing lost (`ActionQueue`, `PendingIntentActions`, `DelayedEventQueue` persist; messages and edges accumulate). N ticks: each message read once, each edge seen by the first tick only, each tick fully drains `ActionQueue`. Event latency inside the tick shrinks from one frame to one tick per hop.

### 5. Tick layout and per-schedule table (B2, B3, round 2)
The D3 sets are configured in `FixedUpdate` as well (`GameplaySchedulePlugin` keeps its **exact registration spot**; WASM ties break by insertion order, and adding sets to `RunFixedMainLoop`/`FixedUpdate` reorders unconstrained systems on the single-threaded web executor: **diff the executor order before and after each phase**). A set's configuration is per schedule: a set with no members and no configured neighbours in a schedule makes every `.before/.after(set)` against it a silent no-op (bevy_ecs 0.18 `Dag::flatten` passes edges through an empty set only if it has neighbours).

| Schedule / slot | Systems (live manifest at `ef169e7`) |
|---|---|
| **Update** (stays) | `spawn_player_when_terrain_ready`, `backend_glue` and `load_backend_config` (until Phase 4), `cursor_capture` (until Phase 4), `preload_audio`, `preload_decals`, `pipeline_warmup`, `audio_state` (a projection of `AudioState`; in `Update` it still runs before audio playback starts in `PostUpdate`, so `mute_on_start` holds), camera chain, animation, nameplates/world labels, `damage_popup`, fading, `clear_pool_on_scene_unload`, audio |
| **Update, render projections** (Phase 4: lose `GameplaySet` membership) | `inventory_ui`, `container_ui`, `slot_menu_sync`, `bar_assign_sync`, `action_bar_visual`, `action_bar_item_slot`, `audio_volume_var`, `drain_dynamic_stat_ui`, `drain_particle_effects`, `simulate_pool`, `rebuild_pool_meshes`, `spawn_decal`, the dialogue panel projection. Note `slot_menu_sync`/`bar_assign_sync` write UI-mode state (`slot_menu`, `assign_mode`) that capture reads: that is allowed (capture reads state as of the last tick), but they stay one-frame-lag projections |
| **BeforeFixedMainLoop, Phase 3** (own sets; **`Scene` first**, then the UI chain in its present order) | `check_project_loaded`, `spawn_scene_v2` (ahead of the capture sets: keeps `schedule.rs`' "Scene -> {Ui, Game}" promise without a gate), `global_input`, `unclaimed_gamepad_trigger`, `button`, `icon_button_click`, `inventory_slot_click`, `bar_assign_click`, (`container_click`), the `SimInput` capture (Phase 2) |
| **BeforeFixedMainLoop, Phase 4** | `click_select`, `aim_target` (-> `TargetClick`/`Aim`), `backend_glue` adapter half (-> `BackendInbound`), `load_backend_config`, `cursor_capture` (+ the `locked` snapshot), the control-tick system |
| **FixedUpdate, always (the control-tick whitelist)** | Begin: latch `SimActive`, drain `CollisionEvent` into a persistent queue, advance `SimTick` (only if active), apply `BackendInbound` -> `Emit` always-run members (`target_auto_clear`, which the shipped pause contract forbids gating, `real_pause.md:109,116`; and `Dialogue`, after `DirectActions` and before `Interpret` so D3's push order `DirectActions -> Dialogue -> FSM` holds; it is a message reader: an in-body gate on the `SimActive` latch, its `UiEvent`/`GameEvent` readers drained first, so a control tick neither loses nor replays `dialogue_choice:*`) -> `Interpret` (`fsm`, `entity_fsm`) -> `Execute` (`flush_pending_intent`, `default_panel_actions`, `action_executor`, `apply_sim_clock`, `stat_effective_value` 2nd instance, `stat_threshold`, `drain_spawn_queue`) -> `PostExecute` (`npc_hit_relay`, `apply_bar_assignments`) -> end: clear `SimInput` edges and every leftover payload. `drain_spawn_queue` is in this row, so queued spawns keep draining while paused at 2 per frame (as today) |
| **FixedUpdate, sim-only** (`run_if SimActive` on the set, **including the Rapier sets**) | movement chain (`gamepad_bind -> input_translator -> player_movement -> view_box_clamp`); `update_player_speed`; `GameStreamSet::World` = `collectible -> trigger_zone -> npc_behavior` (read the persistent collision queue, never the message); `Targeting` (the `TargetClick` applier, then `tab_targeting`, then the `Aim` applier: today's click -> tab -> aim writer order), `ActionBar` (`cooldown_tick`, `action_bar_input`), `Interact`, `DelayedEvents`, `Stats`; `DirectActions` (`resolve_pending_behaviors`, `despawn_timer`); `motion`, `mark_dirty_trees`, **`PhysicsSet::{SyncBackend, StepSimulation, Writeback}`** (third-party sets: `configure_sets(FixedUpdate, PhysicsSet::... .run_if(sim_active))`) |

One tick, in order: Begin -> movement -> `Emit` (**`World` first**, then `Targeting -> ActionBar -> Interact -> DelayedEvents -> Stats`, then `DirectActions`, `Dialogue`; `target_auto_clear` runs ungated after `Targeting`) -> `Interpret` -> `Execute` -> `PostExecute` -> `motion -> mark_dirty_trees` -> Rapier (pipeline **before** `SyncBackend`; `CollisionEvent`s still come from the previous tick's step) -> clear edges. **`World` first keeps the shipped order**: physics-tick events (pickups, trigger zones, NPC reactions) are read before targeting, action bar and interact today (docs/30:386-387; D3 has no `World` set and they come first only because `FixedUpdate` runs before `Update`), so "walk into a trigger zone and press F in the same step" does not flip. D3's principle (input, then delayed, then stat) also holds. A Phase 4 test pins both.

**Dead edges to rewrite** (silently no-op once members move schedule): `dialogue_tick.after(button_system)` and `.after(interactable_system)` (`lib.rs:451-452`); `audio_state_system.before(fsm_interpreter_system)` and D3's `.before(GameplaySet::Interpret)` (moot once `audio_state` stays in `Update`, but the edge must be deleted, not left dead); the by-name `click_select/tab/auto_clear.before(action_bar_input_system)` edge (`targeting.rs:65`, `:89`), which is now the `Targeting -> ActionBar` set order and is covered by the set-target test, **and the implicit `ActionBar` before `Interact` order that lets the cursor-capturing click see `locked == false` (finding 12): not visible to the dead-edge test, so a dedicated test pins it**; the action-bar chain `.after(bar_assign_click_system)` (`action_bar.rs:155`, pinned by a test at `schedule_order_tests.rs:133`); `cursor_capture.after(interactable_system)` (`lib.rs:443`); `target_hud_update_system.after(target_auto_clear_system)` (`lib.rs:481`); `target_indicator_system.after(target_auto_clear).after(aim_target)` (`target_indicator.rs:28-35`); `aim_target.before(first_person_camera_system)` (`targeting.rs:76`); `inventory_ui`/`container_ui`/`npc_hit_relay`/`audio_volume_var`/`apply_bar_assignments` ordered against `action_executor_system` through `PostExecute`; `pipeline_warmup.after(GameplaySet::Execute)`. **Phase 3 adds a multi-schedule test** (`schedule_order_tests::every_ordering_target_resolves_in_its_schedule`) that checks **both** kinds of target in `Update`, `FixedUpdate` and `RunFixedMainLoop`: every system-type target resolves to a system in that schedule, and every *set* target has at least one member or one configured neighbour in that schedule. D3's structure and source-scan tests become multi-schedule; the "`stat_effective_value` registered twice" test (count == 2 in `Update`) breaks in Phase 1 and is rewritten per schedule.

### 6. Pause and `Time<Virtual>` (decision, revised after B1 and round 2; Q1)
**Designer contract: unchanged.** `docs/30` "What freezes / What keeps running" (lines 317-321) is already the spec, and `docs/20` documents that bag-slot clicks and action-bar slot clicks/keys do nothing while paused (docs/20:5143, :1182), that an `EmitEventAfterDelay` waits (docs/20:4049, `delayed_event_while_paused`, `--strict`), and that "do not arm a timer from the pause menu" (docs/30:343-345). Nothing in this plan changes what a designer sees. Two things are true today but undocumented and get one line each in docs/30: dialogue auto-advance **and choice clicks** are frozen, and camera shake is frozen (`real_pause.md:66,103`). **Each capture system keeps its present gate:** `button_system` and `global_input_system` run while held (Resume, Quit); `inventory_slot_click`, `bar_assign_click`, `click_select`, tab-targeting, interact and the action bar stay no-ops while held (their in-body `SimClock` check, or `SimActive` once they are sim systems). Shop Buy and container Take keep today's behaviour; the tension between docs/20:1467 ("add `PauseGame`" when a panel opens) and docs/20:5143 is verified in Phase 3 before anything moves.

**Mechanism.** `SimClock` stays the single owner; `apply_sim_clock_system` stays the only caller of `Time<Virtual>::pause()`. The freeze still stops animation, particles and timers for free (Bevy advances `AnimationPlayer` off `Time<Virtual>`). With the interpreters in `FixedUpdate` a paused clock would starve them (zero ticks), so:
- **One control tick per rendered frame while held**, from an exclusive `BeforeFixedMainLoop` system that calls `world.run_schedule(FixedMain)` directly (verified by the architect: no re-entrancy, works on single-threaded WASM; generic `Time` is `Time<Virtual>`, delta 0; `FixedPostUpdate`'s message-update signal still fires, which makes `keep_messages_flowing_while_held` redundant but harmless; the normal loop then sees a zero delta and runs no ticks).
- **The trigger is `!SimClock::is_running() || Time<Virtual>::is_paused()`** (engine plumbing: an allowed exception to "gameplay never reads `is_paused`"). Revision 2 used `!is_running()` alone, which **deadlocks** after a Replace-mode load: `spawn_scene_v2` (in `Update`) releases the `Menu` hold but only `apply_sim_clock_system` (now in the fixed tick) unpauses `Time<Virtual>`; next frame `SimClock` says running, so no control tick, and the virtual delta is 0, so no normal tick: `apply_sim_clock` never runs and the game stays frozen. With the corrected trigger the control tick runs, `apply_sim_clock` unpauses, and time resumes the frame after (the documented behaviour). **Test: pause, Replace-load, assert ticks resume.**
- **`SimActive = SimClock::is_running() && !Time<Virtual>::is_paused()`, latched once per tick in `Begin`** and read through `resource_equals` (so `SimActive` is `PartialEq` and always initialised by the plugin: minimal test apps need it), never live. So a Resume mid-tick runs **no physics step** that tick (the latch was false; `SimTick` did not advance), and a pause executed in tick k of an N-tick frame turns ticks k+1..N into control ticks (the gate is per tick).
- **The latch is the only defence, not a second one.** Rapier's `TimestepMode::Fixed` steps with its own `dt` and ignores `Time` (`bevy_rapier3d-0.33.0/src/plugin/context/mod.rs:809`), and in ticks k+1..N of a frame generic `Time` is `Time<Fixed>` with dt 1/64 (and `Time<Fixed>::elapsed` keeps advancing). So `PhysicsSet::{SyncBackend, StepSimulation, Writeback}` get the `SimActive` condition explicitly, and the Phase 4 spike **asserts Rapier does not step in a control tick** and that no timer moves.
- **A whitelist, not a blacklist:** only the "always" row runs in a control tick; everything else sits under the `SimActive` set condition (Bevy evaluates a set condition lazily at the first member's turn, once per schedule run, `single_threaded.rs:83-105`, and every `run_schedule(FixedMain)` re-evaluates it, which is exactly the wanted semantics because the latch is constant within a tick; this amends `schedule.rs`' "run conditions stay on their systems", which was about per-system conditions).
- **No gated message reader:** `collectible`/`trigger_zone` read a persistent `CollisionEventQueue` filled by an ungated Begin system (the `NpcHitQueue` pattern); the dialogue sim half is always-run with its in-body gate (table). Any other reader inside a gated set must follow one of the two patterns.
- Spike fallback: `Time::<Fixed>::accumulate_overstep(period)` is **worse** (it advances `Time<Fixed>`). Rejected for v1: a `SimPaused` flag without the `Time<Virtual>` freeze (animation, particles and audio would need separate pause levers; revisit at Beta 0.6, when a networked pause cannot freeze a shared clock anyway).
- While held, rules run once per rendered frame, not 64 times a second: designers are unaffected, but the "1/64 s per hop" claim is worded "while the game runs".
- A frame where `!running` but `Time<Virtual>` is not paused (a hold just taken in `Update`) runs a control tick **and** normal ticks; correct, because every tick latches false.

### 7. The `Execute` split, the spawn cap and dialogue (B2; Q3)
D3's `Execute` mixed sim and render work. **Phase 4 splits it explicitly:** fixed (whitelist) `flush_pending_intent -> default_panel_actions -> action_executor -> apply_sim_clock -> stat_effective_value -> stat_threshold -> drain_spawn_queue`; `Update` (render rate, draining persistent queues) `drain_dynamic_stat_ui -> drain_particle_effects -> simulate_pool -> rebuild_pool_meshes -> spawn_decal`, keeping the `fading_*` `.after` edges. Otherwise particles step at 64 Hz with no interpolation, are not simulated on 0-tick frames, and the mesh rebuild runs up to 16 times in a catch-up frame on single-threaded WASM.
**Spawn cap (Q3, revised):** `drain_spawn_queue_system` (`entity_spawner.rs:353`) is one block covering players, cameras, nameplates and the stat-UI queue, and GLB colliders depend on the scene being instantiated, so a "logical spawn now, render attach later" split is a separate refactor (**own backlog item, not this plan**). Phase 4 does the simple thing: a **per-tick cap of 2 inside the existing drain** (keeps today's wave timing at 60 fps; at most **128 spawns per second on every machine**), and the docs recommend `max_frame_delta_secs` of 0.1 or less, which bounds a post-stall burst (6 ticks x 2 = 12 spawns). At the **default** `max_frame_delta_secs` of 0.25 a stall burst is 16 ticks x 2 = 32 spawns, and a 30 fps machine creates 4 spawns per frame (today 2): a large wave can hitch more on slow web machines, which is the stall the cap exists for, and the docs say so. Designer wording: "spawns happen at up to 128 per second on every machine".
**Dialogue (N4):** `dialogue_tick_system` is split in Phase 4 into the sim half (advance, conditions, actions; always-run, with an in-body gate on the `SimActive` latch and drained readers) and an `Update` projection (panel `Visibility`/`Text`/choice buttons). Phase 1 leaves it in `Update`.

### 8. What stays non-deterministic after this plan
Documented in `docs/dev/40_determinism_and_networking.md` with a one-line designer translation ("the same inputs give the same gameplay events; visuals and load timing can differ per machine"): rendering and animation pose, audio, particle simulation; the **asset-load completion tick** (`SceneEvent::Ready`, `BehaviorHandle`, dialogue assets; harness answer: hold tick 0 until Ready and stamp the tick, or record load-complete as an input); **online replies** (`backend_glue`, finding 10: recorded as inputs); camera-derived input (click-to-target); the gamepad debounce `stable_secs`/`stuck_secs` (the clock is already `Time<Fixed>`; the nondeterminism is hardware connect/disconnect timing, N3); `Entity` indices (never tie-break on them, D4). The harness hashes `{SimTick, LogicState, GameVariables, ActionQueue length, CooldownMap, DelayedEventQueue, LoadedStats, SpawnRegistry keys, rigid-body transforms}` **at end of tick** (after `Writeback`, N7) and reports the first diverging tick; Phase 5 adds the hook and exposes `SimTick` in `DebugState` and the diagnostics HUD so a human or `test_web.py` can compare logs.

### 9. WASM / performance
Per-tick cost: these systems already run once per frame with early-outs. At 60 Hz render the load is about unchanged, at 144 Hz it falls (64 runs/s vs 144), at 30 fps it doubles per frame (2 ticks). Catch-up bursts are bounded by `max_delta` (<= 16 ticks), and the burst is now (physics + interpreters + executor + stat loops) x 16 on one WASM thread: **Phase 4 measures a forced 250 ms stall on `local_coop_demo` (4 players, 4 bars)**, not only a steady 30 fps, and docs recommend `max_frame_delta_secs` around 0.1 for heavy web projects (N6). Watch items: `entity_fsm_interpreter_system`, the `stat_*` loops, `action_bar_input`, the spawn drain, and the **sync points in `FixedUpdate`, which multiply with the number of ticks in a catch-up burst**.

### 10. Tests: blast radius and migration
~707 tests, ~1018 `app.update()` calls. dt-dependent tests are unaffected (one tick per update). The break class is **same-`update()` chains**. With the scene stream ahead of the tick, `SceneEvent::Ready` reactions stay same-update (the deferred `SceneHandleV2` still lands one frame later, as today), which removes most of the churn round 1 feared (N2); still audit the `scene_lifecycle`, `entity_logic` and `local_coop` helpers first (shared, so a fix is cheap but widespread). Use Bevy's `TimeUpdateStrategy::FixedTimesteps(n)` for N-tick tests and `ManualDuration(1/144)` for 0-tick cadence; document the first-`update()`-is-0-ticks rule in `support`. New tests: 0-tick, 2-tick, 4-tick-burst and 144 Hz cadences for each of press, click, delayed event, cooldown, `ActionQueue` drain; a two-`App` test at 30 and 144 Hz cadences comparing event logs per `SimTick` (this is D5 extended; coordinate, do not duplicate). Every `MessageReader` assertion helper written against same-`update()` delivery is in the blast radius. Run per-file (disk rule); `cargo check -p ironhold_cli` each phase.

## Tasks
**Phase 1 (S-M): timers** (Ready: both reviewers; first commit and second commit named)
- [ ] **Commit 1:** `FIXED_DT`, `SimTick`, `advance_sim_tick` in `FixedFirst` (there is no `Begin` set yet; derive from `FIXED_TICK_RATE`); expose `SimTick` in `DebugState`
- [ ] **Commit 2:** insert `tick_delayed_events`, then `stat_modifier -> stat_regen -> stat_effective_value`, plus `cooldown_tick`, into the existing `FixedUpdate` chain **between `npc_behavior` and `motion`** (`lib.rs:416-428`), in D3's stream order (`DelayedEvents` then `Stats`), with an explicit edge against the existing `GameEvent` writers. Pull `cooldown_tick` out of the `ActionBar` tuple (`action_bar.rs:149-159`: its `.chain()` and `.after(bar_assign_click)` apply to all three systems). The emptied `GameStreamSet::DelayedEvents`/`Stats` stay configured in `Update` so edges pass through them. All moved systems read only `Res<Time>` (no `MessageReader`), so there is no gated-reader risk and a held clock (zero ticks) freezes them exactly as today. `despawn_timer` and dialogue stay in `Update`
- [ ] Update `schedule_order_tests`: membership (the manifest names `GameStreamSet::*` in `Update`), the "`stat_effective_value` registered twice" count test per schedule, and the assertions `target_auto_clear -> cooldown_tick` and `cooldown_tick -> action_bar_input -> visual`; the `lib.rs` comments; `capabilities/CLAUDE.md` timer notes
- [ ] State the **interim inversion** in designer words (keys, pad and UI buttons are UI events and are still read first; only the targeting, action-bar and interact game events arrive *behind* delayed and modifier events until Phase 4; **on a slow machine, events from two steps arrive together, in step order**) in docs/30, with one test
- [ ] Tests: delayed event / cooldown / modifier expire on the same `SimTick` at 30/60/144 Hz cadences; a held clock freezes them (zero ticks)
- [ ] `ironhold validate`: an info note for a `delay_secs`/`cooldown_secs` above 0 and below 1/64 s ("rounded up to one step"); no shipped project has one
- [ ] Docs, the lines that become wrong **at Phase 1**: docs/30:387-389 (delayed events and modifier expiry now run ahead of targeting, action bar and interact), :394 (re-check "timers, regeneration, cooldowns start counting one frame later"), :426 ("sees the previous frame's stat regeneration" becomes the same frame), :464-465 ("the backend runs after the delayed events and before the stat events" becomes after both); the `DebugState` field lists at docs/30:59, :603, :688 and `STATUS.md:124`/`:127` (the JSON example) now that `SimTick` is exposed; docs/20:4900 and :4913 (field is **`max_frame_delta_secs`**; cooldowns, delays and regen now fall under the tick budget, so the slow-motion footgun slows them too). A "Timing resolution" note near the docs/20 timer fields naming **only the Phase 1 timers**: durations round up to the next 1/64 s (0.0156 s), a repeating timer re-arms one step after it fires, stat bars and the cooldown sweep update 64 times a second; despawn timers and dialogue move in Phase 4, camera shake, fades and particles stay render-rate
**Phase 2 (M): SimInput for fixed readers only** (Ready: both reviewers, with the amendments below)
- [ ] Reproduce finding 4 as failing tests first (lost jump on a 0-tick frame, doubled on a 2-tick frame)
- [ ] `SimInput` (shape in Approach 3), `BeforeFixedMainLoop` capture, end-of-tick clear, the `InFixedMain` flag, the explicit `FixedLast` ordering of the two clears, the read-outside assertion
- [ ] **Amendment:** capture skips and clears edges while the clock is held or `Time<Virtual>` is paused (test: a jump pressed in the pause menu does not fire after Resume, as today); gamepad edges captured per pad and resolved to the owner in the tick (test: a pad that binds this tick keeps its press); minimal test players without a `SpawnId` are in the blast radius
- [ ] Migrate `input_translator` (and any other reader already in `FixedUpdate`); **no `Update` reader switches here**
- [ ] Docs: `docs/40` "input seam"
**Phase 3 (M-L): capture layer, UI only** (needs round 4; temporarily different: nothing designer-visible; online replies and the cursor lock behave as today because both stay in `Update`)
- [ ] **First, before any move:** the multi-schedule set and dead-edge test (Approach 5); the spike that the scene stream behaves identically in `BeforeFixedMainLoop`; diff the single-threaded executor order
- [ ] Move the scene stream, then `global_input`, `unclaimed_gamepad_trigger`, `button`, `icon_button_click` and the slot-click systems to capture in their own sets, **each keeping its pause gate**; payload slots unchanged (their consumer is still later in the same frame)
- [ ] Tests: a key and a bag click on a Replace-load frame use the new bindings and the released clock; the cursor-capturing click does not fire a mouse-button slot (pins finding 12 while `cursor_capture` stays in `Update`)
- [ ] Verify the docs/20:1467 vs :5143 pause tension; verify the deferred `SceneHandleV2` ordering with a test; playtest the click classes
- [ ] Docs: re-check docs/30:392-395 (the promise is unchanged); the capture-layer rule in `src/CLAUDE.md`
**Phase 4 (L): sim move and pause** (needs round 4; temporarily different: nothing, it is the last of the sim-affecting phases)
- [ ] Spike: the control tick, with assertions that Rapier does not step and no timer moves in it, that a Resume mid-tick runs no physics step, **and that pause -> Replace-load -> resumes**; amend `real_pause.md` (mechanism only: the designer contract is unchanged)
- [ ] Move D3's sets to `FixedUpdate`; `GameStreamSet::World` **first in `Emit`**; split the movement chain; `SimActive` latch and set conditions including the Rapier sets; `CollisionEventQueue`; `despawn_timer`; `update_player_speed`; `target_auto_clear` ungated
- [ ] Targeting capture (`click_select`, `aim_target`) -> `TargetClick`/`Aim` (test that `aim_target` still wins); the payload `Vec`s and the leftover-clear rule (test: two menu clicks across a 0-tick frame carry their own quantities; a join with no bound rule leaves no stale pad); the `backend_glue` split (`BackendInbound`); the `cursor_capture` lock snapshot, all in the same commits as their consumers' moves
- [ ] The `Execute` split, the dialogue split (placed in `Emit` before `Interpret`), the spawn cap of 2 per tick; switch the `Update` readers of `SimInput` in the same commit as their move
- [ ] Test: walking into a trigger zone and pressing F in the same step still orders as today (`World` first)
- [ ] Update `schedule_order_tests` (graph in `FixedUpdate`), `tests/support`, helpers; keep `delayed_event_while_paused`; optionally extend it to an `EmitEventAfterDelay` in a binding whose event is exactly `game.paused` (`first_person_starter/logic/state_machine.ron:34`) and check it treats `TogglePause` like `PauseGame` (`local_coop_demo/logic/state_machine.ron:8`); document the ActionBar `do_actions` blind spot
- [ ] Docs, the lines that become wrong **at Phase 4**: docs/30:207, :223 (the "planned" per tick/frame sentence), :321, :384, :390-395, :406-407, :686; docs/20:1164, :1172, :4420 (threshold read "at the start of the next frame"), :5224 (the item slot "follows the bag the same frame": now a projection with one frame of lag), :5227, :5446; docs/20:3365-3370 (the join pad "only valid for the one frame it was captured": say whether a join reached through an `EmitEvent` chain now keeps its pad); the `max_frame_delta_secs` advice of 0.1 at docs/20:4909 and the table near :4917; the docs/30 "System ordering" block and "Same-frame event order" in tick-neutral words; **define "step" once** near docs/30:370 (the game advances in steps of 1/64 s; on a fast screen several frames share one step, on a slow one a frame runs two); the honest hop-cost sentence ("1/64 s per hop while the game runs; 2.25x slower than a frame on a 144 Hz screen"); the undocumented frozen items (dialogue auto-advance and choices, camera shake); **spawn wording**: docs/30:49, `STATUS.md:94` and `:20` ("Deterministic Tick ... Not implemented"), `spawn_wave_encounter.md:283`, with the 4-spawns-per-frame at 30 fps and 32-spawn default-delta burst caveats; `src/CLAUDE.md` pipeline section and the sim-state/projection rule; `docs/50`, `docs/10`, `docs/40` Milestone A marked done
- [ ] WASM: measure steady 30 fps and a forced 250 ms stall on `local_coop_demo`; diff the single-threaded executor order; count sync points in a 16-tick burst
**Phase 5 (M): leftovers** (planned after Phase 4: "click-select resolver" and "behavior activation tick-gating" have no design yet)
- [ ] `resolve_pending_behaviors` activation tick-gating; click-select resolver; state-hash hook and the expected-divergence list; `docs/40` rewrite (and its overclaims at :109, :218); D4 coordination

## Playtest checklist
Every line is tagged **[browser]** (a human in the web build), **[automated test]** (the cadence tests) or **[144 Hz]** (needs a high-refresh display or the `ManualDuration(1/144)` harness). Any "lands on the same `SimTick`" claim is **[automated test]**: `DebugState` is a snapshot of the current tick, not a per-event log. **Forced low fps in the browser:** Chrome DevTools Performance panel, CPU throttling 4x or 6x on a heavy scene, with the FPS read from the Rendering tab's "Frame Rendering Stats" (the diagnostics HUD is only planned, `planning/features/diagnostics_hud.md`).
- **Phase 1:** [automated test] cooldown, monster respawn (**30 s** in `enemy_*.behavior.ron`) and modifier expiry on the same `SimTick` at 30/60/144; [browser] `entity_logic_demo` `respawning_gem` as a smoke check (the gem still reappears and can be collected again, no stuck state; the inversion test proves the ordering); `stats_demo`/`primitive_world` buffs expire on time, regen looks smooth, threshold events fire; `particles_demo`/`effect_mayhem_demo` flicker loops (the repeating ones are 0.18-0.30 s; the 0.05 s values in `torch`, `star_shower` and `lightning_orb` are one-shot entry delays) keep looping; pause freezes all of them.
- **Phase 2:** [144 Hz] and [browser, throttled] mash jump: no dropped or doubled press on keyboard or gamepad; [automated test] a jump pressed in the pause menu does not fire after Resume.
- **Phase 3:** [browser] quick press-and-release clicks (throttled and at normal fps) on action-bar slots, inventory/shop buttons, container Take/Take All, dialogue choices and the pause Resume button; [144 Hz] `scene_key_bindings`/`global_key_bindings` and the gamepad Start button; [browser] a key and a bag click on the frame a new scene appears; bag-slot clicks, action-bar clicks and keys, world-click targeting and Tab stay no-ops while paused; first person on native: the click that captures the mouse does not fire a mouse-button slot.
- **Phase 4:** [browser] pause: Resume click, Esc, Resume+Esc in one frame, pause during a delayed event (must not fire), `LoadScene` while paused (the deadlock case), a scratch rule that arms `EmitEventAfterDelay`/`SetDespawnTimer` from a pause state waits for resume, `local_coop_demo`'s `shake_camera` key while paused stays frozen, `primitive_world` death while the overlay is up; [automated test] a collision on the last tick before pause is processed after Resume, `aim_target` wins over Tab, two menu clicks in a frame with no tick keep their quantities, a trigger-zone entry and F in the same step order as today; [browser] `local_coop_demo` both players firing in one frame, hot-join with a gamepad, 4-way split, wave spawn timing, dialogue `npc_intro` auto-advance, chest/loot panel and shop; tab away for 5 s and back (a hitch, not a glitch).
- **`python test_web.py`:** expect particle-phase pixel diffs on `particles_demo`, `effect_mayhem_demo`, `primitive_world` (the first delayed `SpawnEffect` can shift by one tick); re-baselining those is allowed, nobody should chase it as a bug. The `?static=1` backend demos must still apply `backend.ready`/`backend.offline` (they stay in `Update` until Phase 4).

## Open questions (answered by Frank 2026-11-19: **all recommendations accepted**; both reviewers' answers recorded)
1. **Pause mechanism (blocks Phase 4).** Both reviewers: keep the `Time<Virtual>` freeze plus the whitelist control tick via `world.run_schedule(FixedMain)`, with the trigger `!running || virtual_paused` and the Rapier sets gated explicitly. `accumulate_overstep` is a worse fallback. Recommended: yes.
2. **Accept the <= 15.6 ms click-to-effect granularity.** Both: accept.
3. **Spawn throttle.** Revised: per-tick cap of 2 inside the existing drain (up to 128 spawns per second on every machine) plus the documented `max_frame_delta_secs` of 0.1 or less; the logical/render split is a separate backlog item. UX withdrew its round-1 "1 per tick"; the architect prefers the simple cap.
4. **Paused delayed events: preserve the shipped contract** (they wait for resume; `delayed_event_while_paused` exists). Revised from "decide": nothing to decide, only an optional extension of the warning to `game.paused` bindings. Recommended: preserve, optional extension yes.
5. **Phase order.** 1 -> 2 -> 3 -> 4 -> 5 (D1-D3 and real pause are shipped), with D4 before Phase 5 and D5 plus the harness after. Phases 1 and 2 start now (round 3: Ready); 3-5 after round 4.

## Acceptance criteria
Tags: **[designer]** can be checked by a designer in the browser; **[engineer]** needs a harness or test; the rest of the list is engineer-only by nature (the deterministic-tick promise).
- **[engineer]** Given identical input streams stamped by `SimTick`, when run at 30, 60 and 144 fps cadences, then every timer-driven event (delayed, cooldown, despawn, regen, modifier expiry, dialogue advance) fires on the same `SimTick`, and the interpreter receives events in D3's order at the same tick.
- **[engineer]** Given a key, click or gamepad press on a frame with 0 ticks, then it is applied by the next tick exactly once; on a frame with 2+ ticks, once, by the first tick; a press made while the clock is held does not fire after Resume.
- **[engineer]** Given a held `SimClock` (Phase 4): no cooldown, despawn, delayed-event, regen, modifier, dialogue or camera-shake timer moves; Rapier does not step; `SimTick` is frozen; Resume/Quit/UI transitions still process in the control tick; a Resume mid-tick runs no physics step that tick; a collision from the tick before the pause is still processed after Resume.
- **[designer]** Given a pause followed by a Replace-mode scene load that releases it, then the game resumes (no deadlock).
- **[designer]** **The shipped pause contract is unchanged** (Phases 3 and 4): bag-slot clicks, action-bar slot clicks and keys, world-click targeting, Tab and interact stay no-ops while paused; Resume and Quit buttons work; shop Buy and container Take behave as today; online replies still arrive while paused (Phases 3 and 4).
- **[engineer]** Given two bag-menu clicks in a frame with no tick, then each action carries its own quantity; a join press keeps its pad; no payload outlives the first tick that reads its events.
- **[designer]** Given a key or click on the frame a new scene appears, then it uses the new scene's bindings and the released clock (Phase 3).
- **[engineer]** Given any system ordering target in `Update`, `FixedUpdate` or `RunFixedMainLoop`, then it resolves to a system, or a set with members or configured neighbours, in that schedule; and the implicit `ActionBar`-before-`Interact` order is pinned by its own test.
- **[engineer]** Given `aim_target` and tab-targeting in the same tick, `aim_target` still wins; given a physics-tick event and a targeting/action-bar/interact event in the same step, the physics event is read first, as today.
- **[designer]** Cosmetic systems are unchanged apart from stat bars and the cooldown sweep updating 64 times a second; no new frame of UI latency beyond the <= 1 tick wait; 30 fps, 144 Hz and a 250 ms stall stay within the agreed budget on `3rd_person_game_demo` and `local_coop_demo`.
- **[engineer]** The existing suite passes after the documented mechanical migration; `ironhold_cli` checks pass; the expected-divergence list is documented in `docs/dev/40_determinism_and_networking.md`.

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

### Resolution of plan-review round 2 (2026-11-19, `79f07b9`)
| Finding | Where it is resolved |
|---|---|
| Architect 1 (control tick deadlocks after a Replace-load) | Approach 6: trigger `!running \|\| virtual_paused`, latch `running && !virtual_paused`; test and acceptance criterion |
| Architect 2 (dt consumers do not stop on their own; Rapier ignores `Time`) | Approach 6: the latch is the only defence; Rapier sets get `SimActive`; spike asserts it |
| Architect 3 (`menu_action` single `Option`, `PendingJoinGamepad`, `backend_glue` writes `GameVariables`, `aim_target` continuous and last writer) | Findings 9-11; Approach 3: `PanelPayloads` FIFO, `BackendInbound`, `Aim` level sample applied after tab-targeting; Phase 3/4 split |
| Architect 4 (dead-edge list and test incomplete; capture sets; control-tick ordering) | Approach 5 list, set-target test, capture sets, `.after` every capture set |
| Architect 5 (dialogue sim half is a gated reader) | Table "always" row, Approach 7 |
| Architect non-blocking (table gaps, capture-reads-state rule, scene-load frame, Phase 1 `despawn_timer` and `cooldown_tick` test breaks, spawn split, `InFixedMain`, executor order, sync points) | Approach 3 rule and scene-load bullet; Phase 1 task and phases table; Approach 7 spawn; Approach 3 flag; Phase 3/4 tasks |
| UX B1 (pause table re-specifies shipped behaviour) | Approach 6 "Designer contract: unchanged"; Q4 reframed; acceptance criterion; `validate` task reduced to an optional extension |
| UX B2 (docs lists stale or missing) | Phase 1, 3, 4, 5 docs bullets with line numbers; spawn wording moved to Phase 4 |
| UX non-blocking (interim-inversion wording, hop cost, spawn wording, `SimTick` in `DebugState` in Phase 1, playtest methods and per-phase split, paused tick cost) | Phase 1 tasks, Approach 6 last bullet, Approach 7, playtest checklist |

### Resolution of plan-review round 3 (2026-11-19, `9084c75`)
| Finding | Where it is resolved |
|---|---|
| Architect B1 / UX B1 (the `backend_glue` split loses replies while paused; hub state shared) | Finding 10: both backend systems stay in `Update` through Phase 3; Phase 4 splits them with explicit hub-field ownership and `BackendInbound`; acceptance criterion |
| Architect B2 (`cursor_capture` ahead of the action bar makes the capturing click fire a slot) | Finding 12; Approach 3 `cursor_capture` bullet; stays in `Update` in Phase 3, lock snapshot in Phase 4; dedicated test |
| Architect B3 (`target_auto_clear` must not be gated) | Always-run row; sim-only row and tick order |
| Architect B4 (payload FIFO has no consumer or expiry rule) | Approach 3 item 3: one rule (`Vec`s, leftover-clear at the end of the first reading tick), `PendingJoinGamepad` as `Vec<(trigger, Entity)>`, the `assign_mode` exception |
| Architect B5 (dialogue and `TargetClick` applier placement) | `Dialogue` in `Emit` before `Interpret`, always-run; `TargetClick` applier at the head of `Targeting` |
| Architect: scene-load-frame gate undefined | Approach 1 and 3: the scene stream moves to `BeforeFixedMainLoop` ahead of the capture sets; no gate; spike in Phase 3 |
| Architect Phase 2 amendments | Phase 2 tasks: skip/clear edges while held, per-pad capture resolved in the tick, `FixedLast` clear order |
| Architect non-blocking (stale citations, `update_player_speed`, set-condition wording, `PartialEq` latch, control-plus-normal tick frame, paused spawn drain) | Approach 5/6/7 edits and the Phase 4 tasks |
| UX B2 (scene-load frame vs docs/30:392-395) | Same scene-stream decision; docs re-check in Phase 3 |
| UX B3 (`World` after targeting flips the shipped order) | `World` first in `Emit`; Phase 4 test; acceptance criterion |
| UX B4 (playtest methods unverifiable) | Playtest tags and methods; same-`SimTick` claims moved to [automated test] |
| UX missed docs lines, N1 hop cost, N2-N5, `validate` info note, 0.05 s correction | Phase 1 and 4 docs bullets, "What", Approach 7, Phase 1 tasks, playtest |

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

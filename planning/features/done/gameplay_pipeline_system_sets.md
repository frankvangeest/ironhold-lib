# Feature: Named Pipeline `SystemSet`s + a Deterministic Order for Every Pre-Interpreter Event Writer

_Status: Ready (refreshed 2026-10-14 against `c6c4161` after real pause, fixed timestep + D1/D2, SetUiVisible and ui_scale
landed; system-architect. Decisions D-a..D-d unchanged; **new D-e..D-h** below, one of which (D-g) replaces the
`PlayerInputSet` task that `real_pause.md` added. Original plan-review 2026-10-02: see "Plan-review outcome")_
_Planned at: `c6c4161` (2026-10-14)_ (originally `87dab15`, 2026-10-01)

Backlog item: **D3** (Beta 0.5 ▸ determinism work). Supersedes and merges the former Queued items
"Bevy ambiguity-detection hardening (`ScheduleBuildSettings`) on `Update`" and "Schedule-graph
assertions + first named `SystemSet`". Source: `planning/investigations/hashmap_iteration_order_audit.md`
(finding A, as corrected by the system-architect triage) and the plan-review of 2026-10-02.

## What
Give the `Update` message → interpreter → executor pipeline a declared, named order. Today it is a pile of
ad-hoc `.before(fsm_interpreter_system)` edges across `lib.rs` and two plugins: the systems that *write*
`UiEvent`/`GameEvent`/`SceneEvent`, or *push to `ActionQueue` directly*, are ordered against the
interpreter but **not against each other**, so the order in which same-frame events and actions reach the
FSM is left to the executor. This feature introduces the repo's first named `SystemSet`s, puts every such
system into a fixed order inside them, and adds tests that fail if a writer/pusher/reader is missing from the
declared structure or a new unordered conflict appears on the message/action/`SimClock` resources. For a designer
nothing is authored differently; same-frame event precedence becomes **documented, identical on every
machine, and identical between native and web**.

## Why
- **Lockstep/replay need identical event order.** The FSM takes the *first matching transition* per event
  and applies it immediately (`fsm_interpreter.rs:90-125`), so which of two same-frame events is read first
  can change the final `LogicState`. With writer order left to the executor, native (multi-threaded) and
  web (single-threaded) can disagree, and two native runs can disagree with each other. This is the
  schedule-order half of the HashMap audit; the data-structure half is D1/D2 (both shipped).
- **The ordering we rely on is currently implicit and fragile.** `capabilities/CLAUDE.md` ("Targeting systems are
  ordered on purpose") documents that "targeting → interpreter" holds only *transitively* through `ActionBarPlugin`'s
  own `.before(fsm_interpreter_system)` edge; remove it and the guarantee vanishes with no failing test. The
  repo still has **zero** named `SystemSet`s (re-verified at `c6c4161`), and a real race (the same-frame
  targeting/action-bar `{target}` bug) went undetected for months, found only by an incidental test flake.
- **Real pause added a new shared resource with unordered readers.** `SimClock` is written by `spawn_scene_v2`
  (Replace auto-resume) and the executor (`PauseGame`/`ResumeGame`/`TogglePause`), and read by seven input gates. Three
  of those reads are unordered against `spawn_scene_v2` today (see Findings), so on the auto-resume frame native and web
  can disagree on whether an interact/targeting press counts.
- **It unblocks the rest of the determinism work.** D4 (`SpawnId` tie-breaks), D5 (two-`App` smoke test)
  and `gameplay_fixed_tick_pipeline.md` (which makes D3 a hard prerequisite of its Phase 1 and re-homes these sets into
  `FixedUpdate` in Phase 3) all want a stable, named place to hang systems.

## Findings (re-verified against code at `c6c4161`, 2026-10-14)
Where the registration lives: **`GamePlugin::build` (`lib.rs:140-447`)**, not `start_app` (`lib.rs:874`), which only adds
window/render/framepace plugins and then `GamePlugin`. The Update registrations are `lib.rs:235-442`. Consequence: the
test helper `tests/support/mod.rs::setup_test_app()` already installs `GamePlugin`, so **a schedule test built on it sees
every `Update`/`FixedUpdate` system** — the original plan's "the schedule test is blind to anything registered inline"
premise was wrong (see D-f).

- Interpreters read **all `UiEvent`s, then all `GameEvent`s, then all `SceneEvent`s**
  (`fsm_interpreter.rs:35-53`, `entity_fsm_interpreter.rs:24-40`), so order *between* message types is
  already fixed; only the order of writers **of the same message type**, of direct `ActionQueue`
  pushers, and of readers vs. writers matters.
- `MessageWriter<T>` takes `ResMut<Messages<T>>`; same-type writers serialise but in an executor-chosen
  order. UI writers and Game writers share no message resource, so they **can** run in parallel today.
- **`UiEvent` writers (`Update`):** `button_system` (`lib.rs:552`), `icon_button_click_system` (`lib.rs:593`),
  `global_input_system` (`input.rs:42`), `unclaimed_gamepad_trigger_system` (`input.rs:94`; since real pause it also
  emits `global_claimed_gamepad_bindings` triggers, `input.rs:148-164`, before its unclaimed/join triggers, at most once
  per trigger per frame).
- **`GameEvent` writers in `Update`:** the stat chain `stat_modifier → stat_regen → stat_effective_value`
  (`lib.rs:263-267`; expiry events `stats.rs:11`), `interactable_system` (`lib.rs:361`), `tick_delayed_events_system`
  (`lib.rs:804`, registered `:375`), `ActionBarPlugin`'s `cooldown_tick → action_bar_input → action_bar_visual` chain
  (`action_bar.rs:99-108`), `TargetingPlugin`'s three chained systems (`targeting.rs:58-68`) — and, *after* the
  interpreters, `flush_pending_intent_system` (`action_bar.rs:360`), `action_executor_system` (`action_executor.rs:37`),
  **`apply_sim_clock_system` (`sim_clock.rs:100`, NEW since `87dab15`: `game.paused`/`game.resumed`, chained right after
  the executor at `lib.rs:292-293` with `run_if(resource_changed::<SimClock>)`)** and `stat_threshold_system`
  (`stats.rs:109`), whose events are therefore read at the start of the **next** frame.
- **`GameEvent` writers in `FixedUpdate`** (owned by the fixed-tick item): `collectible_system` (`collectible.rs:30`),
  `trigger_zone_system` (`trigger_zone.rs:36`), `npc_behavior_system` (`npc.rs:220`), `player_movement_system`
  (`player.rs:451`); all inside the one `FixedUpdate` `.chain()` at `lib.rs:346-356`. A paused clock runs zero ticks, so
  none of them writes while paused.
- **`SceneEvent` writers:** `action_executor.rs:27`, `spawn_scene_v2` (`scene_loader.rs:50`; writes `Loaded` `:87` and
  `Ready` `:1353`) and `check_project_loaded` (`project_loader.rs:53`, now registered **on its own** at `lib.rs:235` with
  `run_if(in_state(AppState::LoadingProject))`; still unordered against the interpreter; `fsm_interpreter.rs:12-19`
  still documents the one-frame jitter).
- **Readers other than the interpreters** (missed by the original inventory):
  - `dialogue_tick_system` (`dialogue.rs:85`; reads `UiEvent` + `GameEvent` `:90-91`, pushes `ActionQueue` `:89`).
  - `npc_hit_relay_system` (`npc.rs:170`, reads `GameEvent`; `.after(action_executor_system)` only, `lib.rs:310`, so it
    is **unordered against `apply_sim_clock_system` and `stat_threshold_system`**, both later `GameEvent` writers).
  - **`SceneEvent` readers, all unordered against both `SceneEvent` writers:** `preload_audio_system`
    (`scene_loader.rs:3625`) and `preload_decals_system` (`:3645`), both in the spawn tuple at `lib.rs:245-246`, and
    `clear_pool_on_scene_unload_system` (`particle_renderer.rs:482`, `lib.rs:305`; also unordered against the pool
    systems in the Execute tail on `ResMut<ParticlePool>`). `update_debug_state` reads `SceneEvent` in `PostUpdate`
    (`lib.rs:825`), out of scope.
- **Direct `ActionQueue` pushers other than the interpreters/executor** (the "Never push to `ActionQueue` from a capability
  system" rule in `src/CLAUDE.md:25` is already broken by these; unchanged since `87dab15`): `dialogue_tick_system`,
  `despawn_timer_system` (`despawn_timer.rs:46`, push `:58`, unordered at `lib.rs:418`) and
  `resolve_pending_behaviors_system` (`entity_spawner.rs:564`, push `:603`, in the unordered spawn tuple at `lib.rs:249`).
  `flush_pending_intent_system` and the executor's own `ShowFloatingText` re-pushes (`action_executor.rs:1547,1569`)
  are inside the interpreter chain and already ordered.
- **`SimClock` (NEW, `runtime/sim_clock.rs`).** Writers: `spawn_scene_v2` via `SceneV2Params.sim_clock`
  (`scene_manager/mod.rs:673`) and `action_executor_system` via `SceneStateParams.sim_clock` (`mod.rs:682`) — both hidden
  inside `SystemParam` bundles, so a signature grep cannot see them. Readers:
  - per-system `run_if(sim_running)`: `interactable_system` (`lib.rs:361`), `click_select_system`/`tab_targeting_system`
    (`targeting.rs:61-62`);
  - in-body `Res<SimClock>`, deliberately *not* `run_if`: `action_bar_input_system` (`action_bar.rs:182-188`; keeps its
    `Ref<Interaction>` change baseline advancing), `dialogue_tick_system` (`dialogue.rs:95,115-117`; only the choice
    branch is gated, panel upkeep keeps running), `first_person_camera_system` (`camera.rs:581-599`; drains the
    `MouseMotion` reader and still positions the camera), `fly_camera_system` (`flycam.rs:63,77-82`; drains the reader);
  - `apply_sim_clock_system` (the only `Time<Virtual>::pause()/unpause()` caller) and `keep_messages_flowing_while_held`
    (`sim_clock.rs:84`, registered in **`Last`**, `lib.rs:363`; touches `MessageRegistry`, not any `Messages<T>`).
  - **Races today:** `interactable_system`, `click_select_system`, `tab_targeting_system` and `action_bar_input_system`
    are unordered against `spawn_scene_v2` (both are only `.before(fsm_interpreter_system)`); `first_person_camera_system`
    and `fly_camera_system` (camera chain, `lib.rs:381-407`) are unordered against **both** writers. `dialogue_tick_system`
    is ordered (before the interpreters) against the executor but not against `spawn_scene_v2`.
- `spawn_scene_v2` also writes `LoadedKeyBindings`/`LoadedGamepadBindings` (Replace branch), read by
  `global_input_system`/`unclaimed_gamepad_trigger_system`; unordered today, so a key pressed on a Replace-load frame
  is evaluated with the old or the new bindings depending on the executor.
- `stat_effective_value_system` is registered **twice** (`lib.rs:266` pre-interpreter, `:294` post-executor). Legal, with one
  rule: Bevy refuses to build if anything orders against that function *by name* while two instances exist
  (`SystemTypeSetAmbiguity`).
- The action-bar cost gate reads `.current`, not `effective` (`action_bar.rs`, `resolve_cost_source`).
- **New `Update` edges since `87dab15`** that D3 must carry over unchanged: `audio_volume_var_system
  .after(action_executor_system).before(update_dynamic_labels_system)` (`lib.rs:276-278`), `ui_scale_system
  .before(split_viewport_player_label_update_system).before(target_hud_update_system)` (`:411-413`), the per-system
  `run_if(sim_running)` gates above, `apply_sim_clock_system` inside the interpreter chain, and the `// load-bearing:`
  markers added by the core CLAUDE.md split (`lib.rs:259,275,284,344,357,366,373,377,419`).
- **Bevy 0.18 facts that shape the design** (verified in `bevy_ecs-0.18.0` source):
  - A **set-level** run condition is evaluated **once per frame, at the first member's turn**, cached
    (`evaluated_sets`), and on `false` every member is skipped for the frame (`executor/single_threaded.rs:86-106`;
    the multi-threaded executor does the same).
  - `conflicting_systems()` compares **system access only** (`schedule/node.rs:607-640`); a `run_if` condition's own
    access (e.g. `sim_running`'s `Res<SimClock>`) is not part of it, so a conflict test must inspect conditions
    separately (`Systems::get_conditions(key)` → `ConditionWithAccess.access`).
  - Per-system access is readable after `schedule.graph_mut().initialize(world)` via `graph().systems.get(key).access`
    (`node.rs:40-45,488`), before `Schedule::initialize` moves the systems into the executable.
  - `before_ignore_deferred`/`after_ignore_deferred`/`chain_ignore_deferred` exist for sets and systems (`config.rs`), so
    a new ordering edge need not add an auto `ApplyDeferred`.
  - The single-threaded (web) executor runs systems in the graph's topological order, produced by a Tarjan-SCC toposort
    over node insertion order **and** edges (`graph/graph_map.rs:368`, `tarjan_scc.rs`). Order of *unconstrained* pairs
    is deterministic but graph-shape-dependent: adding any set node or edge can reorder unrelated unconstrained systems
    on web, wherever the plugin is registered.
- No cycles arise from the proposed edges (re-traced at `c6c4161` against every existing edge: the three plugin edges
  `action_bar.rs:107`, `targeting.rs:66`, `target_indicator.rs:26`; dialogue_tick; `spawn_scene_v2`; `audio_state_system`;
  `audio_volume_var_system`; `ui_scale_system`; `target_hud_update_system`; `damage_popup`/`world_label`/
  `nameplate_visibility`; `inventory_ui`/`container_ui`; `fading_*`; `apply_sim_clock_system`; the `FixedUpdate` chain's
  `.before(PhysicsSet::SyncBackend)`. Nothing in Emit/Interpret/Execute is ordered after any camera-chain system.

## Approach
**1. New module `runtime/schedule.rs`** defining (all `#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]`,
`pub` because the tests need them, documented as unstable):

```rust
pub enum GameplaySet { Emit, Interpret, Execute, PostExecute }
pub enum EmitSet { Scene, Ui, Game, DirectActions, Dialogue }       // all inside GameplaySet::Emit
pub enum GameStreamSet { Targeting, ActionBar, Interact, DelayedEvents, Stats }  // chained inside EmitSet::Game
```

Configured once in a new `GameplaySchedulePlugin`: `Emit → Interpret → Execute → PostExecute` chained in
`Update` (normal edges: the sync point before `Interpret` is required, see D-d). Inside `Emit`:
**`EmitSet::Scene` first** (before both `Ui` and `Game`, normal edges, see D-e); `EmitSet::Ui` and `EmitSet::Game`
independent of each other (they share no message resource); `DirectActions` after `Ui` and `Game`; `Dialogue` after
`Ui`, `Game` and `DirectActions`. New intra-`Emit` edges other than Scene → (Ui, Game) use the `*_ignore_deferred`
variants so they add no sync point (today's command visibility inside `Emit` is unchanged); the `GameStreamSet` chain is
`chain_ignore_deferred()`, and so are the two *new* system chains whose members have `Commands`: `EmitSet::Scene`
(`check_project_loaded → spawn_scene_v2`) and `EmitSet::DirectActions` (`resolve_pending_behaviors_system →
despawn_timer_system`). This does not drop their flush: it lands on the next normal edge (the Scene → {Ui, Game} sync and
the pre-`Interpret` sync). Existing system-level `.chain()`s keep their current (deferred-respecting) form.

**Sync-point accounting.** Bevy inserts an auto `ApplyDeferred` on every *normal* edge whose source has deferred params.
Expected new sync nodes: **one** from Scene → {Ui, Game} (D-e), plus possibly one from `Execute → PostExecute`
(`spawn_decal_system`, last in `Execute`, has `Commands`; today only `.after(action_executor_system)` edges reach the
PostExecute members). Sync nodes are **shared per dependency distance across the whole schedule**, so a new upstream sync
can shift unrelated, hidden orderings: the before/after run-order diff must be *reviewed*, not expected to come out clean.
The D-e sync node runs every frame (native: an exclusive barrier; cheap when nothing is queued).

**2. Membership**

| Set (all inside `GameplaySet::Emit` unless noted) | Systems / order |
|---|---|
| `EmitSet::Scene` (`.chain_ignore_deferred()`) | `check_project_loaded` (keeps its `run_if(in_state(LoadingProject))`) → `spawn_scene_v2` |
| `EmitSet::Ui` (`.chain()`) | `global_input_system` → `unclaimed_gamepad_trigger_system` → `button_system` → `icon_button_click_system` |
| `EmitSet::Game` = `Targeting → ActionBar → Interact → DelayedEvents → Stats` (`GameStreamSet`, chained) | `TargetingPlugin`'s existing inner chain (`in_set(GameStreamSet::Targeting)` on the inner `(click_select, tab_targeting, target_auto_clear).chain()` only, **not** on `debug_selectables_system`; its two per-system `run_if(sim_running)` stay) → `cooldown_tick → action_bar_input → action_bar_visual` (in-body pause gate stays) → `interactable_system` (keeps `run_if(sim_running)`) → `tick_delayed_events_system` → `stat_modifier → stat_regen → stat_effective_value` |
| `EmitSet::DirectActions` (`.chain_ignore_deferred()`) | `resolve_pending_behaviors_system` → `despawn_timer_system` |
| `EmitSet::Dialogue` | `dialogue_tick_system` (reads both streams and pushes actions, so it runs last in `Emit`; keeps its existing `.after(button_system).after(interactable_system)` edges and its in-body choice gate) |
| `GameplaySet::Interpret` (`.chain()`) | `fsm_interpreter_system` → `entity_fsm_interpreter_system` |
| `GameplaySet::Execute` (`.chain()`, today's tuple minus the two interpreters, 11 members) | `flush_pending_intent_system` → `action_executor_system` → **`apply_sim_clock_system`** (keeps `run_if(resource_changed::<SimClock>)`) → `stat_effective_value_system` (2nd instance) → `stat_threshold_system` → `drain_spawn_queue` → `drain_dynamic_stat_ui` → `drain_particle_effects` → `simulate_pool` → `rebuild_pool_meshes` → `spawn_decal` |
| `GameplaySet::PostExecute` | `npc_hit_relay_system`, `inventory_ui_system`, `container_ui_system`, **`audio_volume_var_system`** (keeps `.before(update_dynamic_labels_system)`), **`preload_audio_system`, `preload_decals_system`, `clear_pool_on_scene_unload_system`** (`SceneEvent` readers, see D-h) |

`apply_sim_clock_system` stays exactly where real pause put it: after the executor (the last `SimClock` writer of the
frame; `spawn_scene_v2`, the other writer, is in `EmitSet::Scene`, earlier) and before `stat_threshold_system`.
`audio_state_system` becomes `.before(GameplaySet::Interpret)`. **Never write `.before/.after(
stat_effective_value_system)`** — order against `GameStreamSet::Stats` / `GameplaySet::Execute` instead
(add to `schedule.rs` docs and `src/CLAUDE.md`). `keep_messages_flowing_while_held` **stays in `Last`**, outside every
gameplay set: it must run after every `Update` reader and before `First`'s message update, touches only
`MessageRegistry`, and is not a writer; `schedule.rs` docs point at it, and the fixed-tick plan must keep it in `Last`
when the sets move to `FixedUpdate`.

**3. Resulting order (what the docs must say).**
- **ActionQueue push order within a frame:** `DirectActions` (behavior activation, despawn timers) →
  `Dialogue` → FSM → entity FSM → action-bar flush. So an action-bar slot's built-in `do_actions` run
  *after* every `state_machine.ron` reaction, and dialogue actions run *before* them.
- **UI stream:** exactly the `EmitSet::Ui` chain (keys → gamepad: claimed-pad triggers, then the unclaimed/join trigger →
  buttons → icon buttons; two bound keys pressed together are ordered by D1, not D3). A keyboard pause key and a joined
  pad's pause button pressed in the same frame produce two `toggle_pause` events in that order (the claimed-pad dedupe
  is per map only) and cancel out: deterministic now, not fixed by D3 (state it in the pause docs).
- **Game stream the interpreters read in frame N:** (1) carry-over from frame N-1's `Execute` set —
  `flush_pending_intent` (`action_bar.activated`), `action_executor` (`EmitEvent`, combat, panel events), **then
  `apply_sim_clock` (`game.paused` / `game.resumed`)**, then `stat_threshold` (threshold crossings); (2) events from each
  `FixedUpdate` tick, in that chain's order (`player_movement → collectible → trigger_zone → npc_behavior`; 0, 1 or 2
  ticks per frame, **0 while paused**); (3) the `EmitSet::Game` chain: `Targeting → ActionBar → Interact →
  DelayedEvents → Stats` (all silent while paused: inputs gated, timers frozen by the virtual clock).
- **What this means for pause.** `game.paused`/`game.resumed` are never same-frame events: a `PauseGame` executed in
  frame N is interpreted at the head of frame N+1, after that frame-N action-bar activation and before any threshold
  crossing the same frame-N actions caused (e.g. a lethal `ModifyStat` and a `PauseGame` in one frame: `game.paused`,
  then `stat.player_health.depleted`, both in frame N+1, by which time the FSM is already in `paused` — why
  `primitive_world` has the `from: "paused"` depleted row). The auto-resume on a Replace load releases the clock in frame N's
  `EmitSet::Scene`, so frame N's input gates already see the sim running (D-e) and `game.resumed` arrives at the head of
  frame N+1, one frame after that load's `scene.loaded`. **Time resumes the frame *after* the load:** `Time<Virtual>` is
  only unpaused in frame N's `Execute`, so on the auto-resume frame the edge-triggered gates are live while every
  dt-based system (timers, regen, cooldowns, camera smoothing) still sees a delta of 0. Say it in designer words in the
  pause docs.
- **Scene stream:** `EmitSet::Scene`, then carry-over from the executor.
- Frank's accepted principle — **input events first, then delayed events, then stat events** — holds
  *within each frame's Emit work*. The one exception is the carry-over head, which is causally earlier
  because it was produced by the previous frame's actions. Stat *thresholds* and the pause events belong to that head
  (see Decisions D-a).
- **Accepted consequences** (state them in the docs and in code comments): the action-bar cost gate sees
  the previous frame's regen (`.current`, one frame ≈ 16 ms, deterministic — today it is random);
  regen/expiry threshold crossings are interpreted a frame later; a behavior that finishes loading reacts to
  this frame's events (automatic `ApplyDeferred` before `Interpret`, merged with the sync point `spawn_scene_v2`
  already creates); on a Replace-load frame every input system sees the new scene's bindings, the released clock and
  the already-applied scene teardown/spawn (D-e); **D-h behavior changes:** `npc_hit_relay_system` now relays a
  `stat.*.health.depleted` crossing (written by `stat_threshold_system`) in frame N deterministically, so an NPC's death
  reaches `npc_behavior_system` one frame earlier than the random N/N+1 before; `clear_pool_on_scene_unload_system` now
  runs after the pool systems, so particles spawned in the `LoadScene` frame are always wiped (before, sometimes they
  survived into the next scene).

**4. Plugin placement.** Add `GameplaySchedulePlugin` (from `GamePlugin::build`) at the spot of the first gameplay
registration it replaces (`lib.rs:235`), keep the remaining non-gameplay registrations' relative order, and carry over
**every ordering comment and every `// load-bearing:` marker** (re-pointed at the set when an edge becomes set
membership). This minimises churn but does **not** freeze web order: the toposort is graph-shape-dependent (Findings),
so some unconstrained, non-conflicting-on-pipeline-resources pairs will reorder on web whatever we do. That is
acceptable because the filtered-conflict test proves no such pair touches the pipeline resources; any pair that
conflicts on *other* data is caught by the order diff (Tasks). The plugin owns **every** `Update` system that touches
`Messages<UiEvent>`, `Messages<GameEvent>`, `Messages<SceneEvent>`, `ActionQueue` or writes `SimClock`. `ActionBarPlugin`
and `TargetingPlugin` tag their systems with `in_set(...)` instead of `.before(fsm_interpreter_system)` (the
transitive-ordering note in `capabilities/CLAUDE.md` becomes an explicit edge). No signature changes. Largest tuples:
the camera chain (15, untouched) and `Execute` (11); limit 20. `spawn_scene_v2`'s and `action_executor_system`'s
16-param ceilings are untouched; `dialogue_tick_system` is also at 16 (post real pause), untouched.

**5. Tests (`crates/ironhold_core/tests/schedule_order_tests.rs`, new).** App = `support::setup_test_app()` (full
`GamePlugin`, D-f), then `app.finish(); app.cleanup();`. Read the `Update` (and `FixedUpdate`) schedule through
`world.schedule_scope`: (1) `schedule.graph_mut().initialize(world)` and collect, per `SystemKey`, the system name, its
access and **both** kinds of condition access (the system's own `get_conditions(key)` and the conditions of every set it
belongs to, `system_sets.get_conditions(set_key)`); (2) `schedule.initialize(world)` and read `conflicting_systems()`,
`warnings()`, `systems()` and `graph().systems_in_set(..)`. Put the two-step helper in `tests/support/mod.rs` (D5 reuses
it). Do not `app.update()`. **Test gotchas** (debug-detective, 2026-10-14):
1. `Schedule::initialize` moves systems *and* run conditions (set conditions too) out of the graph, so everything
   access-related must be collected in step 1; afterwards only `schedule.systems()` and `systems_in_set` are usable.
2. A conflict caused by an exclusive system or `AccessConflicts::All` comes back with an **empty** component list: treat
   any empty-list conflict involving a gameplay-set member as a failure, not as "no pipeline resource involved".
3. Shared sync nodes can order a pair incidentally, so assert *found conflicts ⊆ allowlist*, never `==`.
4. `warnings()` covers the whole `Update` schedule, including third-party plugins (hierarchy detection is `Warn`):
   filter to warnings whose nodes are gameplay-set members or the sets themselves.
5. The `SimClock` condition-pass rule needs carve-outs for the writers themselves (`spawn_scene_v2`,
   `action_executor_system`) and for `apply_sim_clock_system`.
- *Structure:* assert set membership and the `Emit → Interpret → Execute → PostExecute`, `Scene → {Ui, Game}` and
  per-stream chains exist; expect **two** instances of `stat_effective_value_system`; assert `schedule.warnings()` is
  empty (catches redundant membership — put systems only in the specific sets, never also directly in
  `GameplaySet::Emit`).
- *Access-based membership (replaces the source scan, D-f):* every `Update` system whose access **writes**
  `Messages<UiEvent|GameEvent|SceneEvent>`, `ActionQueue` or `SimClock`, or **reads** `Messages<UiEvent|GameEvent|SceneEvent>`,
  must be a member of a `GameplaySet` (resource ids via `world.resource_id::<T>()`). This sees writers hidden in
  `SystemParam` bundles (`SceneV2Params`/`SceneStateParams` → `SimClock`) that a signature grep cannot.
- *Filtered conflicts:* `conflicting_systems()` is computed on every build regardless of `ScheduleBuildSettings`
  (`schedule.rs:1201`). Hard-fail on any conflict whose component ids include the three `Messages<T>` ids, `ActionQueue` or
  `SimClock`, in `Update` and in `FixedUpdate`. Do not widen to a blanket warn. **Plus a condition pass**, because
  `conflicting_systems()` ignores condition access: every system whose *system or condition* access reads `SimClock` must
  be ordered after `EmitSet::Scene` and either before `action_executor_system` or after `apply_sim_clock_system`
  (assert via set membership), except an explicit two-entry allowlist — `first_person_camera_system`,
  `fly_camera_system` — that points at the deferred camera-chain follow-up (D-g). The two allowlisted pairs are also
  the only accepted `SimClock` entries in the filtered-conflict check.
- *Gate inventory (replaces the `PlayerInputSet` membership requirement, D-g):* every `Update` system whose access reads an
  input source (`ButtonInput<KeyCode>`, `ButtonInput<MouseButton>`, `Messages<MouseMotion>`, the `Gamepad` component,
  `Interaction` via `Ref`) **and** writes `UiEvent`/`GameEvent`/`ActionQueue`/`PendingIntentActions` or mutates a player
  `Transform` must read `SimClock` in its system or condition access, unless it is on the explicit exception list:
  the "must NOT be gated" UI writers from `real_pause.md` §3 (`global_input_system`, `unclaimed_gamepad_trigger_system`,
  `button_system`, `icon_button_click_system`; `UiEvent` is in the predicate so these entries are not moot) and
  `camera_orbit_system` (`camera.rs` ~187-196: reads `MouseMotion`/`ButtonInput`/`Gamepad` and writes a
  `CharacterController` `Transform`, but every write is dt-scaled, so it is frozen by `Time<Virtual>`). A new ungated
  edge-triggered input system then fails the test with a message pointing at `src/CLAUDE.md`'s pause rule. Detecting "writes
  a player `Transform`" from access means walking `FilteredAccessSet::filtered_accesses` for a `With<CharacterController>`
  filter, which is fiddly: if that or the exception list (more than ~15 entries) gets unwieldy, **fall back to asserting the
  seven gated systems by name** (plus the exception list as documentation).
- *Behavioral:* a few small tests that two same-frame events from two writers reach the interpreter in the
  documented order (one per documented worked example), plus one for the pause carry-over position: `PauseGame` and an
  `action_bar.activated`-producing slot in frame N → frame N+1's interpreter sees `action_bar.activated` before
  `game.paused`. Reuse `pause_tests.rs`'s fixtures rather than duplicating them. The graph tests are the real gate;
  keep these few.
- Note in the `bevy_019_upgrade` plan that the `resource_id`-based filtering will need porting if resources become
  entities in Bevy 0.19.

**6. Non-goals / deferred.** No gameplay-timer changes (fixed-tick item); no re-homing into `FixedUpdate`; no
query sorting (D4); no `HashMap` changes (D1/D2, shipped); no new RON surface; no pause-gate changes (real pause's
per-system and in-body gates stay exactly as shipped). **Deferred to a follow-up** (logged in
the backlog "Schedule-guard follow-ups deferred from D3"): the `CameraChainSet` wrap of the camera chain — **now also
carrying (4): order it `.after(GameplaySet::Execute)` so first-person/flycam look sees the frame's final `SimClock`, and
then drop D3's two-entry allowlist** — and the `PhysicsSet::SyncBackend` registration guard, and widening the ambiguity
filter beyond message/queue/`SimClock` resources.

**7. D5 hand-off** (in-process two-`App` determinism smoke test). Recommendation: **its own branch, immediately after D3**,
not in D3's branch (D3 is M-L and its graph tests already prove the order structurally; D5 is pure test code once the
pieces below exist). D5's real prerequisite is D3 (the multi-threaded executor order), not D4: an in-process fixture with
primitive-only prefabs and no GLB/scene-instancing allocates entities identically in both `App`s, so query/`Entity`
order matches without D4's `SpawnId` sorting. D4 stays a prerequisite of the cross-platform harness. (The backlog's
"_Dep: D1, D4_" on D5 should become "_Dep: D1, D3_; D4 before the cross-platform harness" — a backlog edit outside this
file.) D3 must expose, and D5 consumes:
- `GameplaySet`/`EmitSet`/`GameStreamSet` as `pub` (already planned), and nothing in `GameplaySchedulePlugin` that assumes
  an executor kind, so D5 can build **App A with `ExecutorKind::SingleThreaded` (web's order) and App B with the default
  multi-threaded executor (native)** on `Update` and assert identical streams: the cheapest in-process proxy for
  "native and web agree".
- The schedule-reading helper in `tests/support/mod.rs` (above) so D5 can assert both `App`s built the same graph.
- A documented observation point: a recorder system `.in_set(GameplaySet::Interpret).after(entity_fsm_interpreter_system)`
  reading `MessageReader<UiEvent|GameEvent|SceneEvent>` in read order, and an `ActionQueue` snapshot taken
  `.after(flush_pending_intent_system).before(action_executor_system)` (single-instance systems, so by-name edges are
  legal). D3 documents these two positions in `schedule.rs`; D5 adds the recorder systems in test code only.
- `setup_test_app()`'s `TimeUpdateStrategy::ManualDuration` (already there) so both `App`s tick identically; D5 starts
  its scripted input only after both reach `AppState::InGame` + `scene.ready`, so asset-load timing cannot skew frame
  indices.

## Decisions (system-architect, delegated by Frank, 2026-10-02 — for Frank's morning review)
- **D-a — stat thresholds stay after the executor; document the timing, do not move them.** Moving
  detection into `Stats` would not change latency for `ModifyStat`/`SetStat` crossings (still frame N+1) and
  could *lose* crossings: detection is edge-triggered on `effective` (`stats.rs:127-147`), so same-frame regen
  or modifier expiry could undo a crossing before it is observed. The "exception" is general: everything the
  executor/flush/clock/threshold systems write arrives at the head of the next frame's Game stream. Fix the false
  comment at `lib.rs:261-262` ("threshold crossings are visible in the same frame"). Same-frame regen
  crossings deferred to the fixed-tick plan. (2026-10-14: real pause's death-while-paused fix relies on exactly this
  carry-over timing; still holds.)
- **D-b — two independent chains, not one.** The interpreters read UI before Game, so UI-vs-Game order cannot
  change an FSM outcome; one chain would only serialise writers that parallelise today. Matches the backlog
  entry.
- **D-c — accept that the cost gate sees last frame's regen** (`.current`, ~16 ms, deterministic; modifier
  expiry doesn't affect the gate). Today it is random, not fresh. Moving `Stats` earlier would put expiry
  events ahead of input events and break the accepted principle. Add a comment in `action_bar_input_system`.
- **D-d — accept `DirectActions` before `Interpret`** (always-same-frame, ahead of FSM actions; automatic
  `ApplyDeferred`; a freshly loaded behavior reacts to this frame's events). Strictly more deterministic than
  today's random frame. Name it as a behavior change in the docs.

## Decisions added at the 2026-10-14 refresh (system-architect; Frank to review)
- **D-e — CHANGE: `EmitSet::Scene` runs first in `Emit`** (was: Scene independent of Ui/Game). `spawn_scene_v2` writes
  `SimClock` (auto-resume), `LoadedKeyBindings` and `LoadedGamepadBindings`, all read by `Ui`/`Game` members; with Scene
  independent those reads stay executor-chosen on every Replace-load frame. Use **normal** (sync-inserting) edges
  `Scene → Ui` and `Scene → Game`, not `*_ignore_deferred`: with the sync, a Replace-load frame's input systems see one
  coherent world (new bindings, released clock, old scene despawned, new one spawned); with `ignore_deferred` they would
  see new bindings and a cleared `SpawnRegistry` against old, not-yet-despawned entities. Cost: one sync node earlier in
  `Emit` that runs **every frame** (native: an exclusive barrier; cheap when nothing is queued, and `spawn_scene_v2`
  early-returns on most frames); no lost parallelism worth measuring. Scene's own internal chain is
  `chain_ignore_deferred()` so this stays the only new sync from Scene (see Approach §1 accounting). Docs must say that
  time resumes the frame *after* a load (Approach §3). Behavior change on the load frame only; name it in the docs. Also closes `check_project_loaded`'s jitter
  against `ProjectClaimedGamepadBindings` readers; update `fsm_interpreter.rs:12-19`'s comment, which describes the old
  unordered state.
- **D-f — CHANGE: access-based membership over the full `GamePlugin` replaces the source scan.** The source scan existed
  because the schedule test was thought blind to inline registrations; it is not (all of them are in `GamePlugin::build`,
  which `setup_test_app()` installs). Reading each system's real access is strictly stronger: it catches writers hidden in
  `SystemParam` bundles and needs no regex upkeep. Systems behind a feature not enabled in tests (`inspector`) are out of
  reach either way; none touches the pipeline resources today. Fallback if the full-`GamePlugin` read hits a snag: the
  original source scan.
- **D-g — CHANGE: no `PlayerInputSet`; keep real pause's per-system and in-body gates.** The task `real_pause.md` added
  (a `PlayerInputSet` with `configure_sets(Update, PlayerInputSet.run_if(sim_running))`) does not survive the code: (1) four
  of the seven gated systems are gated **in the body on purpose** (`action_bar_input_system`'s `Ref<Interaction>`
  baseline, `dialogue_tick_system`'s panel upkeep, `first_person_camera_system`'s camera positioning, the two mouse-look
  reader drains) and would break under a set-level `run_if`; (2) a set condition is evaluated once, at the first member's
  turn, and skips every member, so a set spanning `EmitSet::Game` and the camera chain would apply a pre-executor
  `SimClock` snapshot to camera systems that run after a same-frame `PauseGame`; (3) the set would add no ordering. The
  goal behind the task — *a new edge-triggered gameplay input system cannot ship ungated* — is met by the gate-inventory
  test in §5 instead, which also catches a system whose author never heard of the set. The two camera readers' remaining
  race against the executor is a pause-boundary-frame-only, one-frame mouse-look difference; it is allowlisted and moved
  to the deferred `CameraChainSet` follow-up rather than fixed by ordering the 15-system camera chain after `Execute` in
  this feature (that move is acyclic but also shifts `animation_resolver`/`animation_playback` relative to the executor
  and adds a sync point: a visual change that belongs with the camera-chain work and its baselines).
- **D-h — NEW: non-interpreter message readers join `PostExecute`.** `npc_hit_relay_system` (unordered against
  `apply_sim_clock_system`/`stat_threshold_system`), `preload_audio_system`, `preload_decals_system` and
  `clear_pool_on_scene_unload_system` (unordered against both `SceneEvent` writers; the last also against the pool tail)
  would fail the filtered-conflict test as written. After `Execute` they read this frame's `Ready`/`Unloading` and the
  executor's events deterministically; `clear_pool_on_scene_unload_system` then runs after `simulate_pool`/
  `rebuild_pool_meshes`. `audio_volume_var_system` joins too (it is a post-executor mirror; keeps its
  `.before(update_dynamic_labels_system)`).

## Notes: suspected pre-existing bug (log in the backlog; NOT fixed in D3)
- **`ParticlePoolGroups` cleared before the old world is torn down** _(found at `c6c4161` 2026-10-14, debug-detective
  review of this plan; unverified by repro)_. `clear_pool_on_scene_unload_system` clears `ParticlePoolGroups` on
  `SceneEvent::Unloading` (`particle_renderer.rs` ~487-491), but the old scene's `LevelEntity`s are only despawned later,
  in `spawn_scene_v2`'s Replace branch. If the old scene emits the same pool-group key in that window, a new group entity
  is spawned, then despawned at teardown while its key stays in the registry, so that group renders nothing in the new
  scene. Reproduce: a scene with a continuous emitter, `LoadScene` into a scene using the same effect; the effect is
  invisible. Candidate fix: clear the registry at teardown (or validate group entities on lookup). D-h's move of the
  system to `PostExecute` does not change the window.

## Review log
- 2026-10-02: plan-review (system-architect + ux-gamedesigner-reviewer), folded in below.
- 2026-10-14: refresh against `c6c4161` (system-architect): D-e..D-h added.
- 2026-10-14: debug-detective adversarial review of the refresh: no decision reversed. Applied: sync-point accounting
  (`chain_ignore_deferred` for Scene and DirectActions, expected sync count, shared sync nodes, per-frame D-e barrier);
  gate-inventory fixes (`UiEvent` in the predicate, `camera_orbit_system` exception, by-name fallback); five D-f test
  gotchas; "time resumes the frame after a load" and D-h's two behavior changes for the docs; `in_set(Targeting)` on
  the inner chain only; web-order dump method + proxy caveat; the `ParticlePoolGroups` suspected bug above.

## Plan-review outcome (folded in, 2026-10-02)
Both reviewers returned "needs more design work"; every blocking item is resolved above.
**Architect:** B1 direct pushers → `DirectActions`; B2 dialogue is a reader → `Dialogue` last in `Emit`;
B3 plugin ownership + source-scan test (2026-10-14: the scan is replaced by the access-based check, D-f); B4 two chains +
cost-gate staleness named; N1 verified test API; N2 never order by `stat_effective_value_system`; N5 plugin at the exact
spot (2026-10-14: still minimise churn, but it does not freeze web order; see §4); N6 `SceneEvent` in scope; N4 deferred
(above). **UX:** U1 threshold timing documented as the carry-over rule (D-a); U2 plan, backlog D3 entry and docs/30
reconciled to the single order in Approach §3; U3 mis-targeted playtest case replaced; U4 designer-facing docs section
added to Tasks; U5 pre-existing `game_over` death-while-paused bug — **fixed by real pause** (`primitive_world`
`from: "paused"` depleted row, `logic/state_machine.ron:148-149`; `3rd_person_game_demo` death animation moved to
`global_on`, `:49`), so the logging task is dropped. Remaining reviewer notes (N3, N7-N10) are doc/test/naming fixes
inside D3's files (N3 cycle trace → Findings; N7 cross-frame read order → Approach §3 and docs; N8 → sets add build-time
nodes only, no per-frame cost, but count new sync points (Tasks); N9 → Bevy 0.19 note in §5; N10 → behavioral tests
kept small).

## Tasks
- [x] Re-verify the writer/reader/pusher inventory and reconcile against Findings (done at the 2026-10-14 refresh;
      re-run the greps once more on the branch's base commit before coding, and diff against Findings)
- [ ] **Before touching registrations:** dump the `Update` schedule's executable order with
      `ExecutorKind::SingleThreaded` (web's order) from a `setup_test_app()` build, save it to the scratchpad, and repeat
      after the change (`schedule.systems()` after `initialize` gives the executable order, `ApplyDeferred` nodes
      included). Diff: every pair that swapped and conflicts on any data is either now explicitly ordered or reviewed as
      harmless; count auto-inserted `ApplyDeferred` nodes before/after and justify each new one (expected: one from
      Scene → {Ui, Game}, plus one from `Execute → PostExecute` if it materialises). Shared sync nodes mean the diff will
      not be clean: review it. **Caveat:** the `setup_test_app()` (MinimalPlugins) app is a proxy — no UI/picking/gizmo/
      audio/render systems, no `inspector`/`webgpu` features. For the real web order, a temporary wasm-only exclusive
      system in `Last` (gated to frame >= 2) can print the `Update` schedule's `systems()` names to the console; remove it
      before committing. Diagnostic only, not a committed test.
- [ ] `runtime/schedule.rs`: `GameplaySet`, `EmitSet`, `GameStreamSet` + module docs (unstable; never order
      against `stat_effective_value_system` by name; `keep_messages_flowing_while_held` stays in `Last`; the two D5
      observation points)
- [ ] `GameplaySchedulePlugin` added from `GamePlugin::build` at `lib.rs:235`; carry over all comments and
      `// load-bearing:` markers (re-point them at the sets); `Scene → {Ui, Game}` normal edges, other intra-`Emit` edges
      `*_ignore_deferred`
- [ ] Tag `TargetingPlugin`/`ActionBarPlugin` systems with sets; replace their `.before(fsm_interpreter_system)`
      with set membership; `Targeting → ActionBar` explicit through the `GameStreamSet` chain; keep every
      `run_if(sim_running)`, `run_if(any_action_slots)` and the `CooldownMap` condition on its system (never move a
      condition to a set)
- [ ] Move `check_project_loaded`/`spawn_scene_v2`, `resolve_pending_behaviors_system`, `despawn_timer_system`,
      `dialogue_tick_system` into the Emit sets; split today's interpreter `.chain()` into `Interpret` + `Execute`
      (keep `apply_sim_clock_system` and its condition in place); move `npc_hit_relay_system`, `inventory_ui_system`,
      `container_ui_system`, `audio_volume_var_system`, `preload_audio_system`, `preload_decals_system`,
      `clear_pool_on_scene_unload_system` into `PostExecute`; `audio_state_system` → `.before(GameplaySet::Interpret)`
- [ ] Fix the false comment at `lib.rs:261-262`; update `fsm_interpreter.rs:12-19` (check_project_loaded is now ordered);
      add the cost-gate staleness comment in `action_bar_input_system`; update the "Never push to `ActionQueue` from a
      capability system" rule in `src/CLAUDE.md:25` to list the sanctioned `ActionQueue` pushers; update `src/CLAUDE.md`'s
      "`lib.rs` ordering edges are load-bearing" list and its pause paragraph (gates stay per-system; the gate-inventory
      test is the guard)
- [ ] `tests/schedule_order_tests.rs` (structure + access-based membership + filtered conflicts incl. `SimClock` and the
      condition pass + gate inventory + few behavioral tests, incl. the pause carry-over position); schedule-reading helper
      in `tests/support/mod.rs`; add the file to `crates/ironhold_core/tests/CLAUDE.md` and the root `CLAUDE.md` test loop
- [ ] Docs: rewrite `docs/30_runtime_events_and_logic.md` "System ordering" (~L944) as **"Same-frame event
      order"** in designer language (no system/set names): lead with *design so order doesn't matter*; the
      stream order of Approach §3; push order (dialogue, then FSM, then action bar); two worked examples
      (e.g. Resume click + Esc in one frame — key first, so Esc leaves `paused` and the Resume transition no longer
      matches; and Esc + a `playing`-scoped click is silently dropped → put must-not-lose reactions in
      `global_on`); promote the "timers/thresholds/NPC/physics events keep arriving in any state — handle in
      `global_on` or author the transition from every state" rule; cross-link "Pausing the game" (~L271: `game.paused`/
      `game.resumed` arrive the frame after the action; keyboard + pad pause in one frame cancel out) and
      "Per-event evaluation order" (~L338), and update "Ordering & determinism notes" (~L379). Add one-line pointers from
      `docs/20`'s key-binding (~L110, L165-166) and `stats.ron` sections (D1 owns its own docs line)
- [ ] Update `runtime/scene_manager/CLAUDE.md` ("Entity FSM and the interpreter chain") and `capabilities/CLAUDE.md` (the
      "Targeting systems are ordered on purpose" transitive-ordering paragraph — now explicit; "Dialogue system" ordering
      line)
- [ ] Reconcile the backlog D3 entry's order text and line citations with Approach §3; extend the "Schedule-guard
      follow-ups deferred from D3" entry with item (4) (camera chain after `Execute`, drop the allowlist); change D5's
      dependency per §7
- [ ] Log the `ParticlePoolGroups` suspected bug (Notes) under `## Bugs` in `planning/backlog.md`
- [ ] Mark the `real_pause.md`-sourced `PlayerInputSet` idea as superseded by D-g (one line in this plan suffices; do not
      edit `features/done/real_pause.md`)
- [ ] `cargo check -p ironhold_cli`; full suite one file at a time (disk rule), including `pause_tests` and
      `same_frame_order_tests`
- [ ] Reviews after implementation: alignment, system-architect, debug-detective; ux-gamedesigner-reviewer
      (docs); wasm-perf-reviewer (single-threaded executor order, the D-e sync point)
- [ ] WASM dev build + playtest checklist below

## Playtest checklist
- Bind **one physical key** to both a `global_key_bindings` UI binding and an action-bar slot: the UI reaction
  must always happen first, identically on native and web.
- Interact with a dialogue NPC while pressing a skill key: dialogue actions run before the slot's actions.
- A behavior-bearing `Action::Spawn` prefab: its entry actions fire with no visible one-frame gap.
- Pause overlay (`3rd_person_game_demo`/`primitive_world`): Resume click + Esc in one frame (Esc first → unpaused, no
  re-pause); pause/resume still freezes and restores everything as in real pause's checklist (no free heal).
- `primitive_world`: get killed and pause in the same moment → game over is still reached.
- `local_coop_demo`: both players press skill keys in the same frame (per-player bars still fire both); joined-pad
  `Start` pause still works.
- `3rd_person_game_demo`: targeting + skill use + death/respawn flow unchanged; Retry from game over (a Replace load
  while paused) resumes cleanly; `python test_web.py` baselines unchanged or deliberately re-baselined with an
  explanation.
- Compare native and web builds for the first case — they must now agree.

## Open questions
None blocking (resolved by Decisions). `EmitSet`/`GameplaySet` are `pub` (tests need them) and documented as
unstable; module lives at `runtime/schedule.rs`; the schedule test hard-fails. For Frank's review: D-e..D-h (two of
them reverse earlier plan text: Scene is no longer independent of Ui/Game, and there is no `PlayerInputSet`).

## Acceptance criteria
- Given any frame in which two `UiEvent` writers (or two `GameEvent` writers) emit, the interpreters read
  them in the documented order, identically on native and web, across repeated runs.
- Every `Update` system that writes `UiEvent`/`GameEvent`/`SceneEvent`/`SimClock`, reads a message stream, or pushes
  `ActionQueue` sits in a named set; the access-based membership test fails for one that doesn't.
- Given a new unordered conflict on those resources (including a `run_if` condition reading `SimClock`), the
  filtered-conflict test fails; the only accepted `SimClock` exceptions are the two allowlisted camera systems.
- Given a new edge-triggered gameplay input system that does not read `SimClock`, the gate-inventory test fails.
- `game.paused`/`game.resumed` arrive at the documented carry-over position (after the previous frame's executor events,
  before its threshold crossings); real pause's existing tests (`pause_tests.rs`) pass unchanged.
- The `Targeting → ActionBar` order is an explicit edge, and the existing same-frame targeting behavior is
  unchanged.
- The docs, this plan and the backlog state one identical order.
- The existing test suite passes with no regression; `ironhold_cli` checks; the release playtest passes and
  screenshot baselines are unchanged or deliberately refreshed.

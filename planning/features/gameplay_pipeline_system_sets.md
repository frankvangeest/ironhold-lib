# Feature: Named Pipeline `SystemSet`s + a Deterministic Order for Every Pre-Interpreter Event Writer

_Status: Ready (plan-review 2026-10-02: system-architect + ux-gamedesigner-reviewer both returned "needs more design work"; all blocking items folded in below on the system-architect's decisions — see "Plan-review outcome")_
_Planned at: `87dab15` (2026-10-01)_

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
system into a fixed order inside them, and adds tests that fail if a writer/pusher is missing from the
declared structure or a new unordered conflict appears on the message/action resources. For a designer
nothing is authored differently; same-frame event precedence becomes **documented, identical on every
machine, and identical between native and web**.

## Why
- **Lockstep/replay need identical event order.** The FSM takes the *first matching transition* per event
  and applies it immediately (`fsm_interpreter.rs:91-121`), so which of two same-frame events is read first
  can change the final `LogicState`. With writer order left to the executor, native (multi-threaded) and
  web (single-threaded) can disagree, and two native runs can disagree with each other. This is the
  schedule-order half of the HashMap audit; the data-structure half is D1/D2.
- **The ordering we rely on is currently implicit and fragile.** `crates/ironhold_core/src/CLAUDE.md`
  documents that "targeting → interpreter" holds only *transitively* through `ActionBarPlugin`'s own
  `.before(fsm_interpreter_system)` edge; remove it and the guarantee vanishes with no failing test. The
  repo has **zero** named `SystemSet`s, and a real race (the same-frame targeting/action-bar `{target}`
  bug) went undetected for months, found only by an incidental test flake.
- **It unblocks the rest of the determinism work.** D4 (`SpawnId` tie-breaks), D5 (two-`App` smoke test)
  and the "gameplay timers onto the fixed tick" item all want a stable, named place to hang systems; the
  fixed-tick plan should re-home these sets into `FixedUpdate` rather than re-derive edges.

## Findings (verified against code at `87dab15`/`28d80de`)
- Interpreters read **all `UiEvent`s, then all `GameEvent`s, then all `SceneEvent`s**
  (`fsm_interpreter.rs:35-52`, `entity_fsm_interpreter.rs:24-40`), so order *between* message types is
  already fixed; only the order of writers **of the same message type** — and of direct `ActionQueue`
  pushers — matters.
- `MessageWriter<T>` takes `ResMut<Messages<T>>`; same-type writers serialise but in an executor-chosen
  order. UI writers and Game writers share no message resource, so they **can** run in parallel today.
- **`UiEvent` writers (`Update`):** `button_system` (`lib.rs:504`), `icon_button_click_system` (`:545`),
  `global_input_system` (`input.rs:41`), `unclaimed_gamepad_trigger_system` (`input.rs:96`).
- **`GameEvent` writers in `Update`:** the stat chain `stat_modifier → stat_regen → stat_effective_value`
  (`lib.rs:244-248`; expiry events `stats.rs:11`), `interactable_system`, `tick_delayed_events_system`
  (`lib.rs:751`), `ActionBarPlugin`'s `cooldown_tick → action_bar_input → action_bar_visual` chain,
  `TargetingPlugin`'s three chained systems — and, *after* the interpreters, `flush_pending_intent_system`
  (`action_bar.rs:340`), `action_executor_system` (`action_executor.rs:37`) and `stat_threshold_system`
  (`stats.rs:112`), whose events are therefore read at the start of the **next** frame.
- **`GameEvent` writers in `FixedUpdate`** (owned by the fixed-tick item): `collectible_system`,
  `trigger_zone_system`, `npc_behavior_system`, `player.rs:451`.
- **`SceneEvent` writers:** `action_executor.rs:27`, `scene_loader.rs:50` (`spawn_scene_v2`) and
  `project_loader.rs:39` (`check_project_loaded`, `lib.rs:217` — unordered against the interpreter today;
  `fsm_interpreter.rs:15-19` already admits the one-frame jitter).
- **Direct `ActionQueue` pushers other than the interpreters/executor** (the "only interpreters push
  `ActionQueue`" rule in `src/CLAUDE.md:30` is already broken by these): `dialogue_tick_system`
  (`dialogue.rs:89`; also a **reader** of `UiEvent` and `GameEvent`, `:90-91`), `despawn_timer_system`
  (`despawn_timer.rs:48`, unordered at `lib.rs:367`) and `resolve_pending_behaviors_system`
  (`entity_spawner.rs:569`, in the unordered tuple at `lib.rs:231`).
- `stat_effective_value_system` is registered **twice** (`lib.rs:247` pre-interpreter, `:262`
  post-executor). Legal, with one rule: Bevy refuses to build if anything orders against that function *by
  name* while two instances exist (`SystemTypeSetAmbiguity`).
- The action-bar cost gate reads `.current`, not `effective` (`action_bar.rs:314-326`).
- No cycles arise from the proposed edges (traced against every existing edge: dialogue_tick,
  `spawn_scene_v2`, `audio_state_system`, `target_hud_update_system`, `damage_popup`/`world_label`/
  `nameplate_visibility`, `inventory_ui`/`container_ui`, `fading_*`, `PhysicsSet::SyncBackend`).

## Approach
**1. New module `runtime/schedule.rs`** defining (all `#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]`,
`pub` because the tests need them, documented as unstable):

```rust
pub enum GameplaySet { Emit, Interpret, Execute, PostExecute }
pub enum EmitSet { Ui, Game, Scene, DirectActions, Dialogue }       // all inside GameplaySet::Emit
pub enum GameStreamSet { Targeting, ActionBar, Interact, DelayedEvents, Stats }  // chained inside EmitSet::Game
```

Configured once in a new `GameplaySchedulePlugin`: `Emit → Interpret → Execute → PostExecute` chained in
`Update`; `EmitSet::Ui`, `EmitSet::Game`, `EmitSet::Scene` independent of each other (they share no
message resource); `DirectActions` after the three, `Dialogue` after `Ui`, `Game` and `DirectActions`.

**2. Membership**

| Set (all inside `GameplaySet::Emit` unless noted) | Systems / order |
|---|---|
| `EmitSet::Ui` (`.chain()`) | `global_input_system` → `unclaimed_gamepad_trigger_system` → `button_system` → `icon_button_click_system` |
| `EmitSet::Game` = `Targeting → ActionBar → Interact → DelayedEvents → Stats` (`GameStreamSet`, chained) | `TargetingPlugin`'s existing chain → `cooldown_tick → action_bar_input → action_bar_visual` → `interactable_system` → `tick_delayed_events_system` → `stat_modifier → stat_regen → stat_effective_value` |
| `EmitSet::Scene` (`.chain()`) | `check_project_loaded` (keeps its `run_if`) → `spawn_scene_v2` |
| `EmitSet::DirectActions` (`.chain()`) | `resolve_pending_behaviors_system` → `despawn_timer_system` |
| `EmitSet::Dialogue` | `dialogue_tick_system` (reads both streams and pushes actions, so it runs last in `Emit`) |
| `GameplaySet::Interpret` | `fsm_interpreter_system` → `entity_fsm_interpreter_system` |
| `GameplaySet::Execute` | `flush_pending_intent_system` → `action_executor_system` → `stat_effective_value_system` (2nd instance) → `stat_threshold_system` → the existing drain/pool/decal tail (chain segment kept as is) |
| `GameplaySet::PostExecute` | `npc_hit_relay_system`, `inventory_ui_system`, `container_ui_system` |

`audio_state_system` becomes `.before(GameplaySet::Interpret)`. **Never write `.before/.after(
stat_effective_value_system)`** — order against `GameStreamSet::Stats` / `GameplaySet::Execute` instead
(add to `schedule.rs` docs and `src/CLAUDE.md`).

**3. Resulting order (what the docs must say).**
- **ActionQueue push order within a frame:** `DirectActions` (behavior activation, despawn timers) →
  `Dialogue` → FSM → entity FSM → action-bar flush. So an action-bar slot's built-in `do_actions` run
  *after* every `state_machine.ron` reaction, and dialogue actions run *before* them.
- **UI stream:** exactly the `EmitSet::Ui` chain (keys → unclaimed gamepad → buttons → icon buttons; two
  bound keys pressed together are ordered by D1, not D3).
- **Game stream the interpreters read in frame N:** (1) carry-over from frame N-1's `Execute` set —
  `flush_pending_intent` (`action_bar.activated`), `action_executor` (`EmitEvent`, combat, panel events),
  then `stat_threshold` (threshold crossings); (2) events from each `FixedUpdate` tick, in that chain's order
  (`player_movement → collectible → trigger_zone → npc_behavior`; 0, 1 or 2 ticks per frame); (3) the
  `EmitSet::Game` chain: `Targeting → ActionBar → Interact → DelayedEvents → Stats`.
- **Scene stream:** `EmitSet::Scene`, then carry-over from the executor.
- Frank's accepted principle — **input events first, then delayed events, then stat events** — holds
  *within each frame's Emit work*. The one exception is the carry-over head, which is causally earlier
  because it was produced by the previous frame's actions. Stat *thresholds* belong to that head, not to
  `Stats` (see Decisions D-a).
- **Accepted consequences** (state them in the docs and in code comments): the action-bar cost gate sees
  the previous frame's regen (`.current`, one frame ≈ 16 ms, deterministic — today it is random);
  regen/expiry threshold crossings are interpreted a frame later; a behavior that finishes loading reacts to
  this frame's events (automatic `ApplyDeferred` before `Interpret`, merged with the sync point
  `spawn_scene_v2` already creates).

**4. Plugin placement.** Add `GameplaySchedulePlugin` at the **exact spot** of the current inline block in
`start_app` (`lib.rs:217-334`) and keep the block's internal order and **every ordering comment** — on
WASM's single-threaded executor ties break by insertion order, so moving registrations would reorder
unconstrained pairs that D3 does not touch. The plugin owns **every** `Update` system that touches
`Messages<UiEvent>`, `Messages<GameEvent>`, `Messages<SceneEvent>` or `ActionQueue`. `ActionBarPlugin` and
`TargetingPlugin` tag their systems with `in_set(...)` instead of `.before(fsm_interpreter_system)` (the
transitive-ordering note in `src/CLAUDE.md` becomes an explicit edge). No signature changes; largest tuple
stays 12 (limit 20); `spawn_scene_v2`'s 16-param ceiling is untouched.

**5. Tests (`crates/ironhold_core/tests/schedule_order_tests.rs`, new).** App = `MinimalPlugins` +
`GameplaySchedulePlugin` + `ActionBarPlugin` + `TargetingPlugin`, with `add_message`/`init_resource` for the
message types and `ActionQueue` so their `ComponentId`s exist. Use `initialize`, not `app.update()`.
- *Structure:* `schedule.graph_mut().initialize(world)`, then assert set membership and the
  `Emit → Interpret → Execute → PostExecute` and per-stream chains exist; expect **two** instances of
  `stat_effective_value_system`; assert `schedule.warnings()` is empty (catches redundant membership — put
  systems only in the specific sets, never also directly in `GameplaySet::Emit`).
- *Filtered conflicts:* `conflicting_systems()` is computed on every build regardless of
  `ScheduleBuildSettings` (`schedule.rs:1201`; no ambiguity-detection setting needed). Hard-fail on any
  conflict whose component ids include `world.resource_id::<Messages<UiEvent|GameEvent|SceneEvent>>()` or
  `ActionQueue`. Do not widen to a blanket warn (zero `SystemSet`s today would flood).
- *Source scan, `ron_lint`-style:* every fn in `crates/ironhold_core/src` whose signature contains
  `MessageWriter<UiEvent|GameEvent>`, `MessageReader<UiEvent|GameEvent>`, `MessageWriter<SceneEvent>` or
  `ResMut<ActionQueue>` must be registered through the plugin or a plugin the schedule test loads; a small
  explicit allowlist covers the `FixedUpdate` writers (`collectible`, `trigger_zone`, `npc_behavior`,
  `player`). This closes the gap that the schedule test is blind to anything registered inline in
  `start_app`.
- *Behavioral:* a few small tests that two same-frame events from two writers reach the interpreter in the
  documented order (one per documented worked example). The graph test is the real gate; keep these few.
- Note in the (future) `bevy_019_upgrade` plan that the `resource_id`-based filtering will need porting if
  resources become entities in Bevy 0.19.

**6. Non-goals / deferred.** No gameplay-timer changes (fixed-tick item); no re-homing into `FixedUpdate`; no
query sorting (D4); no `HashMap` changes (D1/D2); no new RON surface. **Deferred to a follow-up** (logged in
the backlog): the `CameraChainSet` wrap of the camera chain and the `PhysicsSet::SyncBackend` registration
guard (both unrelated to event order; the latter needs `FixedUpdate` in the test), and widening the
ambiguity filter beyond message/queue resources.

## Decisions (system-architect, delegated by Frank, 2026-10-02 — for Frank's morning review)
- **D-a — stat thresholds stay after the executor; document the timing, do not move them.** Moving
  detection into `Stats` would not change latency for `ModifyStat`/`SetStat` crossings (still frame N+1) and
  could *lose* crossings: detection is edge-triggered on `effective` (`stats.rs:127-147`), so same-frame regen
  or modifier expiry could undo a crossing before it is observed. The "exception" is general: everything the
  executor/flush/threshold systems write arrives at the head of the next frame's Game stream. Fix the false
  comment at `lib.rs:242-243` ("threshold crossings are visible in the same frame"). Same-frame regen
  crossings deferred to the fixed-tick plan.
- **D-b — two independent chains, not one.** The interpreters read UI before Game, so UI-vs-Game order cannot
  change an FSM outcome; one chain would only serialise writers that parallelise today. Matches the backlog
  entry.
- **D-c — accept that the cost gate sees last frame's regen** (`.current`, ~16 ms, deterministic; modifier
  expiry doesn't affect the gate). Today it is random, not fresh. Moving `Stats` earlier would put expiry
  events ahead of input events and break the accepted principle. Add a comment in `action_bar_input_system`.
- **D-d — accept `DirectActions` before `Interpret`** (always-same-frame, ahead of FSM actions; automatic
  `ApplyDeferred`; a freshly loaded behavior reacts to this frame's events). Strictly more deterministic than
  today's random frame. Name it as a behavior change in the docs.

## Plan-review outcome (folded in)
Both reviewers returned "needs more design work"; every blocking item is resolved above.
**Architect:** B1 direct pushers → `DirectActions`; B2 dialogue is a reader → `Dialogue` last in `Emit`;
B3 plugin ownership + source-scan test; B4 two chains + cost-gate staleness named; N1 verified test API;
N2 never order by `stat_effective_value_system`; N5 plugin at the exact spot; N6 `SceneEvent` in scope;
N4 deferred (above). **UX:** U1 threshold timing documented as the carry-over rule (D-a); U2 plan, backlog
D3 entry and docs/30 reconciled to the single order in Approach §3; U3 mis-targeted playtest case replaced;
U4 designer-facing docs section added to Tasks; U5 pre-existing `game_over` bug logged under `## Bugs` (not
fixed here). Remaining reviewer notes (N3, N7-N10) are doc/test/naming fixes inside D3's files and are
folded in where they apply (N3 cycle trace → Findings; N7 cross-frame read order → Approach §3 and docs; N8 →
no per-frame cost, sets add build-time nodes only; N9 → Bevy 0.19 note in §5; N10 → behavioral tests kept
small).

## Tasks
- [ ] Re-verify the writer/reader/pusher inventory with `grep` for `MessageWriter<…>`, `MessageReader<…>`,
      `ResMut<ActionQueue>` and reconcile against Findings (add anything found missing)
- [ ] `runtime/schedule.rs`: `GameplaySet`, `EmitSet`, `GameStreamSet` + module docs (unstable; never order
      against `stat_effective_value_system` by name)
- [ ] `GameplaySchedulePlugin` placed at the exact spot of the current inline block; carry over all comments
- [ ] Tag `TargetingPlugin`/`ActionBarPlugin` systems with sets; replace their `.before(fsm_interpreter_system)`
      with set membership; keep `Targeting.before(ActionBar)` explicit
- [ ] Move `check_project_loaded`/`spawn_scene_v2`, `resolve_pending_behaviors_system`, `despawn_timer_system`,
      `dialogue_tick_system`, `npc_hit_relay_system`, `inventory_ui_system`, `container_ui_system` into sets;
      `audio_state_system` → `.before(GameplaySet::Interpret)`
- [ ] Fix the false comment at `lib.rs:242-243`; add the cost-gate staleness comment in
      `action_bar_input_system`; update `src/CLAUDE.md:30` to list the sanctioned `ActionQueue` pushers
- [ ] `tests/schedule_order_tests.rs` (structure + filtered conflicts + source scan + few behavioral tests);
      add it to `crates/ironhold_core/tests/CLAUDE.md` and the root `CLAUDE.md` test loop
- [ ] Docs: rewrite `docs/30_runtime_events_and_logic.md` "System ordering" (~L868-884) as **"Same-frame event
      order"** in designer language (no system/set names): lead with *design so order doesn't matter*; the
      stream order of Approach §3; push order (dialogue, then FSM, then action bar); two worked examples
      (e.g. Resume click + Esc in one frame — key first, so Esc unpauses and the Resume transition no longer
      matches; and Esc + a `playing`-scoped click is silently dropped → put must-not-lose reactions in
      `global_on`); promote the "timers/thresholds/NPC/physics events keep arriving in any state — handle in
      `global_on` or author the transition from every state" rule; cross-link `docs/30` ~L186 and update the
      "Ordering & determinism notes" (~L309-313). Add one-line pointers from `docs/20`'s key-binding and
      `stats.ron` sections (D1 owns its own docs line)
- [ ] Update `src/CLAUDE.md` ("The interpreter chain", the "targeting→interpreter ordering is transitive"
      paragraph — now explicit)
- [ ] Reconcile the backlog D3 entry's order text with Approach §3
- [ ] Log the pre-existing U5 bug under `## Bugs`: `primitive_world/logic/state_machine.ron:130`
      (`from: "playing"` only) and `3rd_person_game_demo/logic/state_machine.ron:220` — dying while paused never
      reaches game over / drops the death animation
- [ ] `cargo check -p ironhold_cli`; full suite one file at a time (disk rule)
- [ ] Reviews after implementation: alignment, system-architect, debug-detective; ux-gamedesigner-reviewer
      (docs); wasm-perf-reviewer (single-threaded executor)
- [ ] WASM dev build + playtest checklist below

## Playtest checklist
- Bind **one physical key** to both a `global_key_bindings` UI binding and an action-bar slot: the UI reaction
  must always happen first, identically on native and web.
- Interact with a dialogue NPC while pressing a skill key: dialogue actions run before the slot's actions.
- A behavior-bearing `Action::Spawn` prefab: its entry actions fire with no visible one-frame gap.
- Pause overlay: Resume click + Esc in one frame (Esc first → unpaused, no re-pause).
- `local_coop_demo`: both players press skill keys in the same frame (per-player bars still fire both).
- `3rd_person_game_demo`: targeting + skill use + death/respawn flow unchanged; `python test_web.py`
  baselines unchanged or deliberately re-baselined with an explanation.
- Compare native and web builds for the first case — they must now agree.

## Open questions
None blocking (resolved by Decisions). `EmitSet`/`GameplaySet` are `pub` (tests need them) and documented as
unstable; module lives at `runtime/schedule.rs`; the schedule test hard-fails.

## Acceptance criteria
- Given any frame in which two `UiEvent` writers (or two `GameEvent` writers) emit, the interpreters read
  them in the documented order, identically on native and web, across repeated runs.
- Every `Update` system that writes `UiEvent`/`GameEvent`/`SceneEvent` or pushes `ActionQueue` sits in a named
  set; the source scan fails for one that isn't registered through the plugin.
- Given a new unordered conflict on those resources, the filtered-conflict test fails.
- The `Targeting → ActionBar` order is an explicit edge, and the existing same-frame targeting behavior is
  unchanged.
- The docs, this plan and the backlog state one identical order; the U5 bug is logged.
- The existing test suite passes with no regression; `ironhold_cli` checks; the release playtest passes and
  screenshot baselines are unchanged or deliberately refreshed.

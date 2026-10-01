# Feature: Named Pipeline `SystemSet`s + a Deterministic Chain of Same-Type Event Writers

_Status: Draft_
_Planned at: `87dab15` (2026-10-01)_

Backlog item: **D3** (Beta 0.5 ▸ determinism work). Supersedes and merges the former Queued items
"Bevy ambiguity-detection hardening (`ScheduleBuildSettings`) on `Update`" and "Schedule-graph
assertions + first named `SystemSet`". Source: `planning/investigations/hashmap_iteration_order_audit.md`
(finding A, as corrected by the system-architect triage) and the system-architect triage of 2026-10-01.

## What
Give the `Update` message → interpreter → executor pipeline a declared, named order. Today it is a
pile of ad-hoc `.before(fsm_interpreter_system)` edges across `lib.rs` and two plugins; the many systems
that *write* `UiEvent`/`GameEvent` are ordered against the interpreter but **not against each other**, so
the order in which same-frame events reach the FSM is left to the executor. This feature introduces the
repo's first named `SystemSet`s, puts every pre-interpreter event writer into a fixed chain inside them,
and adds a schedule test that fails if a writer is missing from the chain or a new unordered conflict
appears on the message/action resources. For a designer nothing changes except that same-frame event
precedence becomes **documented, identical on every machine, and identical between native and web**.

## Why
- **Lockstep/replay need identical event order.** The FSM takes the *first matching transition* per event
  and applies it immediately (`fsm_interpreter.rs:91-121`), so which of two same-frame events is read
  first can change the final `LogicState`. With writer order left to the executor, native (multi-threaded)
  and web (single-threaded) can disagree, and two native runs can disagree with each other. This is the
  schedule-order half of the HashMap audit's findings; the data-structure half is D1/D2.
- **The ordering we rely on is currently implicit and fragile.** `crates/ironhold_core/src/CLAUDE.md`
  documents that "targeting → interpreter" ordering holds only *transitively* through `ActionBarPlugin`'s
  own `.before(fsm_interpreter_system)` edge; remove that edge and the guarantee silently vanishes with no
  failing test. The repo has **zero** named `SystemSet`s. A real race (the same-frame targeting/action-bar
  `{target}` bug) went undetected for months and was found only by an incidental test flake.
- **It unblocks the rest of the determinism work.** D4 (`SpawnId` tie-breaks), D5 (two-`App` smoke test)
  and the "move gameplay timers onto the fixed tick" item all want a stable, named place to hang their
  systems; the fixed-tick plan should re-home these sets into `FixedUpdate` rather than re-derive edges.

## Findings (verified against code at `87dab15`)
- Interpreters read **all `UiEvent`s, then all `GameEvent`s, then all `SceneEvent`s** (`fsm_interpreter.rs:35-52`,
  `entity_fsm_interpreter.rs:24-40`). Order *between* message types is therefore already fixed; only the
  order of writers **of the same message type** matters. (This narrows the audit's finding A.)
- `MessageWriter<T>` takes `ResMut<Messages<T>>`, so same-type writers can never run in parallel —
  they serialise — but their *relative order* is whatever the executor picks. Imposing a chain therefore
  costs **no parallelism** that exists today.
- **`UiEvent` writers (all `Update`):** `button_system` (`lib.rs:504`), `icon_button_click_system`
  (`lib.rs:545`), `global_input_system` (`input.rs:41`), `unclaimed_gamepad_trigger_system` (`input.rs:96`).
  Registered at `lib.rs:233-241`; only `.before(fsm_interpreter_system)` on two of them.
- **`GameEvent` writers in `Update`:** the stat chain `stat_modifier_system → stat_regen_system →
  stat_effective_value_system` (`lib.rs:244-248`; `stats.rs:11` expiry events), `interactable_system`
  (`interactable.rs:48`, `lib.rs:325`), `tick_delayed_events_system` (`lib.rs:751`, `:334`),
  `ActionBarPlugin`'s `(cooldown_tick_system, action_bar_input_system, action_bar_visual_system).chain()`
  (`action_bar.rs:167`), `TargetingPlugin`'s three chained systems (`targeting.rs:198/282/377`, ordered
  `.before(action_bar_input_system)`), and — *after* the interpreters — `action_executor_system`
  (`action_executor.rs:37`), `flush_pending_intent_system` (`action_bar.rs:340`) and `stat_threshold_system`
  (`stats.rs:112`, events land next frame).
- **`GameEvent` writers in `FixedUpdate`** (out of scope here, owned by the fixed-tick item):
  `collectible_system`, `trigger_zone_system`, `npc_behavior_system`, `player.rs:451`.
- `SceneEvent` writers (`action_executor.rs:27`, `project_loader.rs:39`, `scene_loader.rs:50`) are lifecycle
  events, already sequenced by the loading state machine and `spawn_scene_v2.before(fsm_interpreter_system)`.
- Known non-writer pre-interpreter systems with their own edge to the interpreter: `spawn_scene_v2`,
  `audio_state_system`, `dialogue_tick_system` (`.after(button_system).after(interactable_system)`).

## Approach
**1. New module `runtime/schedule.rs`** (crate-internal naming, exported for tests) defining:

```rust
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameplaySet { Emit, Interpret, Execute, PostExecute }

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum EmitSet { UiInput, Targeting, ActionBar, Interact, DelayedEvents, Stats }
```

Configured once in a new `GameplaySchedulePlugin` (see step 4):
`GameplaySet::Emit → Interpret → Execute → PostExecute` chained in `Update`, and `EmitSet` chained
*inside* `Emit` in the order above.

**2. Membership**

| Set | Systems |
|---|---|
| `EmitSet::UiInput` (internal `.chain()`) | `global_input_system` → `unclaimed_gamepad_trigger_system` → `button_system` → `icon_button_click_system` |
| `EmitSet::Targeting` | `TargetingPlugin`'s existing three-system chain (unchanged internally) |
| `EmitSet::ActionBar` | `cooldown_tick_system → action_bar_input_system → action_bar_visual_system` (existing chain) |
| `EmitSet::Interact` | `interactable_system` |
| `EmitSet::DelayedEvents` | `tick_delayed_events_system` |
| `EmitSet::Stats` | `stat_modifier_system → stat_regen_system → stat_effective_value_system` (existing chain) |
| `GameplaySet::Interpret` | `fsm_interpreter_system`, `entity_fsm_interpreter_system` (kept in that order) |
| `GameplaySet::Execute` | `flush_pending_intent_system`, `action_executor_system`, `stat_effective_value_system`, `stat_threshold_system` (existing chain segment) |
| `GameplaySet::PostExecute` | `npc_hit_relay_system` (currently `.after(action_executor_system)`); the pool/decal/spawn-drain tail of the existing chain stays chained after `Execute` |

Non-writers that today say `.before(fsm_interpreter_system)` (`spawn_scene_v2`, `audio_state_system`,
`dialogue_tick_system`) become `.before(GameplaySet::Interpret)`; `dialogue_tick_system` additionally
`.after(EmitSet::UiInput).after(EmitSet::Interact)` (preserving its current `.after(button_system)
.after(interactable_system)`).

**3. Chosen same-frame order (decided in principle by Frank, 2026-10-01: input events first, then delayed
events, then stat events).** `EmitSet` order `UiInput → Targeting → ActionBar → Interact → DelayedEvents →
Stats` — player-input-derived events precede time-derived and state-derived ones, and the existing
`Targeting → ActionBar` guarantee (the documented race fix) is preserved as an explicit edge instead of a
transitive one. Within the interpreters the stream order UI → Game → Scene is unchanged. The *exact*
position of `Interact` and `DelayedEvents` relative to each other is arbitrary-but-fixed; it only needs to
be deterministic and documented (see Open questions).

**4. Testability: extract `GameplaySchedulePlugin`.** `lib.rs` registers these systems inline in
`start_app`'s builder chain (`lib.rs:216-378`), so a schedule test cannot reach them, and
`tests/support::setup_test_app()` deliberately builds a minimal world. Move the *ordering-relevant*
registrations (the `Emit`/`Interpret`/`Execute`/`PostExecute` members above) into a plugin that
`start_app` adds and the schedule test can add to a `MinimalPlugins` app together with the capability
plugins whose systems it orders. Visual/camera/animation systems stay where they are.

**5. The schedule test (`crates/ironhold_core/tests/schedule_order_tests.rs`, new)** — two parts:
- *Structure:* assert every system that takes `MessageWriter<UiEvent>`/`MessageWriter<GameEvent>` and is
  registered in `Update` before the interpreter belongs to a named `EmitSet`, and that
  `Emit.before(Interpret).before(Execute).before(PostExecute)` plus the `EmitSet` chain exist
  (graph query of the built `Update` schedule). This is the "schedule-graph assertion" the old backlog item
  asked for; it also guards `PhysicsSet::SyncBackend` having systems in `FixedUpdate` (the old item's second
  half) and wraps the `Update` camera `.chain()` in a `CameraChainSet` so
  `world_label_screen_pos_system.after(CameraChainSet)` replaces `.after(camera_blend_system)`.
- *Ambiguity:* build the `Update` schedule with ambiguity detection and fail only on conflicts whose
  `ComponentId`s include `Messages<UiEvent>`, `Messages<GameEvent>` or `ActionQueue` — **not** a blanket
  `LogLevel::Warn`, which with zero `SystemSet`s today would flood with unrelated pre-existing ambiguities
  (that wider triage stays a separate follow-up). Exact Bevy 0.18 API (`ScheduleBuildSettings`,
  `Schedule::graph().conflicting_systems()`, component-id lookup) to be verified against the vendored
  `bevy_ecs` source during implementation, not assumed here.

**6. Non-goals.** No gameplay-timer changes (that is the fixed-tick item); no re-homing into `FixedUpdate`;
no change to the Update-vs-`FixedUpdate` event hand-off; no sorting of query iteration (that is D4); no new
RON surface.

## Behavior change and risk
- **Visible only on same-frame collisions.** Native's and web's current orders differ and are not defined, so
  *any* fixed order changes behavior for someone. Concrete collision cases to name in the playtest checklist:
  pause key + inventory key in one frame (state-scoped bindings; whichever is read first decides whether the
  other still matches); interact press + a UI button press in one frame; a `delayed` event firing the same
  frame a stat threshold event does; two bound keys pressed together. Rare in practice, but each should be
  consciously checked, not discovered.
- **Needs the release playtest and screenshot baselines re-checked** (`python test_web.py`); expect no
  baseline diff, but ordering changes on web are exactly the kind of thing screenshots may not show.
- **Plugin refactor risk:** moving registrations into `GameplaySchedulePlugin` must not change any
  system's *relative* order versus the camera/animation chain; the existing ordering comments in `lib.rs`
  must be carried over, not dropped.
- **16-param ceiling:** none of this touches `spawn_scene_v2`'s signature.
- **Not a determinism guarantee by itself:** query-iteration order inside a system (D4) and
  `HashMap` order (D1/D2) are separate. This plan only fixes *which system runs first*.

## Tasks
- [ ] Verify the writer inventory with `grep` for `MessageWriter<UiEvent>`/`MessageWriter<GameEvent>` and
      reconcile against the Findings above (add any writer found missing from the membership table)
- [ ] `runtime/schedule.rs`: `GameplaySet`, `EmitSet`, `CameraChainSet`
- [ ] `GameplaySchedulePlugin`: `configure_sets` + move the ordering-relevant registrations out of
      `start_app`; keep every existing ordering comment
- [ ] Tag `TargetingPlugin` and `ActionBarPlugin` systems into `EmitSet::Targeting`/`ActionBar`; replace their
      `.before(fsm_interpreter_system)` with set membership; keep `Targeting.before(ActionBar)` explicit
- [ ] Replace remaining `.before(fsm_interpreter_system)`/`.after(...)` edges on non-writers with set edges
- [ ] `tests/schedule_order_tests.rs` (structure + filtered-ambiguity); register it in
      `crates/ironhold_core/tests/CLAUDE.md` and in the root `CLAUDE.md` one-file-at-a-time test loop
- [ ] Behavioral tests: for each stream (UI, Game) emit two same-frame events from two writers and assert the
      interpreter sees them in the documented order, repeated across N `App` instances
- [ ] Update `crates/ironhold_core/src/CLAUDE.md` ("The interpreter chain" and the "targeting→interpreter
      ordering is transitive" paragraph — now explicit) and add the same-frame ordering rule to
      `docs/30_runtime_events_and_logic.md`
- [ ] `cargo check -p ironhold_cli`; full test suite one file at a time (disk rule)
- [ ] Reviews: alignment, system-architect, debug-detective; ux-gamedesigner-reviewer (docs changed);
      wasm-perf-reviewer (schedule shape on single-threaded WASM)
- [ ] WASM dev build + playtest checklist below

## Playtest checklist (draft)
- `3rd_person_game_demo`: pause (Esc) and open inventory (I) pressed within one frame; interact with the
  merchant while a skill key is pressed; confirm no regression in targeting + skill use + death/respawn flow.
- `local_coop_demo`: both players press skill keys in the same frame (per-player bars still fire both).
- Compare native and web builds for the pause+inventory same-frame case — they must now agree.
- `python test_web.py` baselines unchanged (or deliberately re-baselined with an explanation).

## Open questions
- Exact order of `Interact` vs. `DelayedEvents` vs. `Stats` (arbitrary-but-fixed; Frank accepted
  "input, then delayed, then stat" — confirm `Interact` belongs with input).
- Should `dialogue_tick_system` and `npc_hit_relay_system` move into the sets, or stay with plain edges? (This
  plan puts `npc_hit_relay_system` in `PostExecute`; `dialogue_tick_system` keeps plain edges.)
- Module/crate naming (`runtime/schedule.rs` vs. `schedule/`), and whether `EmitSet` should be public API or
  `pub(crate)`.
- Whether the filtered-ambiguity test should hard-fail now or start as `warn` for one release.

## Acceptance criteria
- Given any frame in which two `UiEvent` writers (or two `GameEvent` writers) emit, then the interpreters
  read them in the documented `EmitSet` order, identically on native and on web, across repeated runs.
- Given a new system that writes `GameEvent`/`UiEvent` and is not in an `EmitSet`, then the schedule test fails.
- Given the existing targeting/action-bar behavior (the same-frame `{target}` fix), then it is unchanged and
  the `Targeting → ActionBar` order is an explicit edge, not a transitive one.
- Given the full existing test suite, then it passes with no regression; `ironhold_cli` still checks.
- Given the release playtest, then the named collision cases behave per the documented order and the
  screenshot baselines are unchanged or deliberately refreshed.

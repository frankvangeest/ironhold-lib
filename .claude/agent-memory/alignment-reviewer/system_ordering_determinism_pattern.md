---
name: system-ordering-determinism-pattern
description: How to alignment-review a pure Bevy system-ordering/.chain() fix — what counts as designer-visible, the interpreter-chain ordering map, and the deferred-spawn/auto-clear edge this class of fix makes deterministic
metadata:
  type: project
---

Reviewing a change that only adds `.chain()`/`.before()`/`.after()` to existing systems: there is
no schema, no RON, no new capability — so the alignment question is narrowed to **"does making the
order deterministic change any behavior a designer can already reach from RON, and should that new
behavior have been a RON knob?"** The answer is almost always "no knob" — designers must never
order systems; that is exactly the kind of engine internal the data-driven promise depends on being
fixed and invisible. Confirm it as ALIGNED and spend the review budget on the second-order effects
below instead.

**The `Update` ordering map to reason against** (`lib.rs`): everything ordered
`.before(message_interpreter_system)` (global_input, interactable, dialogue_tick,
tick_delayed_events, stat pipeline, audio_state, action_bar chain, spawn_scene_v2,
unclaimed_gamepad_trigger) is the "pre-interpreter input tier". The interpreter chain itself is
`message_interpreter → fsm_interpreter → entity_fsm_interpreter → flush_pending_intent →
action_executor → stat_effective_value → stat_threshold → drain_spawn_queue →
drain_dynamic_stat_ui → drain_particle_effects → simulate_pool → rebuild_pool_meshes →
spawn_decal`, all `.chain()`ed. Adding `.before(X)` where X is already
`.before(message_interpreter_system)` transitively pulls the new systems into the pre-interpreter
tier — **check for and call out that transitive consequence**, because it means any `GameEvent`
those systems emit becomes guaranteed same-frame-visible to all three interpreters instead of
sometimes slipping to the next frame. That is a real (if benign) designer-visible latency change
and usually deserves a docs/30 line, since docs/30 already documents this guarantee explicitly for
`tick_delayed_events_system`.

**The deferred-spawn edge this class of fix keeps exposing.** `Action::Spawn` does *not* spawn
inline — it queues a `QueuedSpawn` drained by `drain_spawn_queue_system` at a hard
`SPAWNS_PER_FRAME = 2`. So an entity's `SpawnRegistry` entry can lag its `Action::Spawn` by one or
more frames whenever a rule queues 3+ spawns. Any pre-interpreter-tier system that treats "not in
`SpawnRegistry`" as "gone" (canonically `target_auto_clear_system`) will therefore deterministically
act on a just-spawned id before it is registered. Before an ordering fix this was a coin flip; after
it is a guaranteed outcome. Flag it as a WARNING, not a blocker — the honest fix belongs in the
spawn/registry path, not in the ordering.

**Test-fixture fallout is the tell that the fix is real.** A correct ordering fix usually forces
previously-flaky test fixtures to become deterministically red, because the fixtures were relying on
the "wrong" order winning some of the time. Expect (and want to see) fixtures gaining the production
invariant they were missing — e.g. `SpawnRegistry` registration for `Targetable`/`ClickSelectable`
entities, since production always gets it via `tag_spawned_entity`. A shared helper plus a comment
explaining *why* (rather than N inline inserts) is the pattern to reinforce; see
`local_coop_tests.rs::spawn_targetable_at`.

**What is NOT worth flagging:** hardcoded engine constants that the ordering fix merely touches
(`SPAWNS_PER_FRAME`, `SELECT_PIXEL_RADIUS`), and the cross-capability import a `.before()` requires
(e.g. `targeting.rs` importing `action_bar_input_system`). The coupling is worth a one-line
architecture note but is not an alignment issue; a named `SystemSet` would be cleaner but is
system-architect's call, not the designer-reachability rubric's.

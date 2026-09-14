---
name: schedule-ordering-mechanism
description: How this codebase expresses Update-schedule ordering (bare function-name .before/.after, zero SystemSets, no ambiguity detection) and why WASM's single-threaded executor makes ordering fixes a web behavior change
metadata:
  type: project
---

**This codebase has zero `SystemSet`s and zero `ScheduleBuildSettings`/ambiguity detection** (verified 2026-09-14: `grep -rn "SystemSet\|configure_sets\|in_set(\|ambiguity_detection" crates/ironhold_core/src` returns nothing). Every ordering constraint is a bare `.before(some_fn)`/`.after(some_fn)` against a concrete system function's `SystemTypeSet`. `lib.rs` alone has ~8 separate `.before(message_interpreter_system)` edges (spawn_scene_v2, global_input, unclaimed_gamepad_trigger, the stat chain, audio_state, interactable, the action-bar chain, …).

**Why:** it's the lowest-ceremony mechanism and it's the established convention — a new `.before(fn)` edge is *consistent*, not a smell in isolation. The smell is the aggregate: a `PipelineSet::PreInterpreter` set would collapse those ~8 edges into one named contract.

**How to apply:**
- Adding a `.before(concrete_fn)` edge is the right call for a targeted ordering fix here; don't demand a `SystemSet` for a single edge. Do note the two costs: (a) the constraint lives on a function identity, so it silently evaporates if that fn is renamed/re-registered/wrapped, and (b) it only names *one* consumer, so it under-specifies an invariant that actually has several consumers.
- **Transitive edges are load-bearing and invisible.** E.g. targeting is ordered before all three interpreters *only* via `targeting → action_bar_input_system → (ActionBarPlugin's own `.before(message_interpreter_system)`)`. Delete the middle edge and the outer one vanishes with no compile error and no test failure. Always trace two hops when reviewing an ordering change.
- **`.before()`/`.chain()` between systems that already conflict on data costs zero parallelism.** Check the param lists first: if both take `ResMut<X>` (or `&mut C` vs `&C` on the same component), the executor was already serializing them and the edge only picks *which* order. This was true for all of `click_select_system`/`tab_targeting_system`/`target_auto_clear_system`/`action_bar_input_system` (all touch `PlayerTarget` + `MessageWriter<GameEvent>`). Say so explicitly when someone asks about the perf cost of a `.chain()`.
- **An ordering fix is a WASM behavior change, not a no-op.** Native runs Bevy's multi-threaded executor (order varies run to run); wasm32 runs the single-threaded executor, where unconstrained systems execute in topological/insertion order — i.e. plugin registration order in `lib.rs`. So web already had a *stable* order, and adding an edge can invert it on web while merely de-randomizing it on native. Flag this for the playtest checklist and for screenshot baselines whenever an ordering edge lands.

See [[schedule-update-vs-fixedupdate]] for which systems live in which schedule, and [[flaky-test-trust-gate]] for the ambiguity-detection follow-up.

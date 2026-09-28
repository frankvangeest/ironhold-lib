---
name: project-rewrite-target
description: rewrite_target()/rewrite_self() string-substitute per action pushed (not per-frame); live in action_substitution.rs since rules.ron removal; interpreter chain cost model
metadata:
  type: project
---

`rewrite_target`, `rewrite_self`, `intent_slot_key`, `scene_path_stem` live in `crates/ironhold_core/src/runtime/scene_manager/action_substitution.rs` (moved out of the deleted `message_interpreter.rs` by `rules_to_state_machine_consolidation`, reviewed 2026-09-28). `action_bar.rs` imports `rewrite_target`.

**Why:** `{target}`/`{self}` substitution. Called by `fsm_interpreter_system`, `entity_fsm_interpreter_system`, and `action_bar_input_system` before `action_queue.push`. `message_interpreter_system`/`LoadedRules`/`Action::EnterState` no longer exist — rules.ron content is now FSM `global_on`.

**How to apply:**
- Cost is `.replace()` allocations PER ACTION PUSHED — only when a binding fires. NOT per-frame. SetVariable now substitutes key AND value (one extra alloc, action-frequency only).
- Interpreter chain steady state: each interpreter builds a `Vec<String>` of this frame's events; empty frames = zero-cap Vec, no alloc. Per-event cost = one format!/clone per interpreter (2 now, was 3).
- `scene_path_stem` returns `Option<String>` (an extra `.to_string()` vs main's `&str`) — per-scene-transition only, irrelevant.
- Real (pre-existing) hotspot: `entity_fsm_interpreter_system` does `binding.event.replace("{self}", id)` for every entity x event x binding/transition + `fsm_state.current.clone()` per entity x event. Only on frames with events, but scales with behavior-entity count (zombie waves). Fix if it ever shows up: precompute per-entity resolved patterns at behavior-resolve time, or compare without allocating (split pattern on "{self}" and match prefix/id/suffix).
- `StateMachineAsset::validate()` now runs per behavior spawn (HashSet alloc) — per-spawn, negligible.

Link: [[project-targeting-capability]] (sets CurrentTarget).

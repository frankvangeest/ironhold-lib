---
name: rules-vs-state-machine-coexistence
description: rules.ron/LogicRulesAsset/message_interpreter_system/EnterState REMOVED by feature/rules_to_state_machine_consolidation (reviewed 2026-09-28); state_machine.ron is the only logic dialect. Lists the leftover traps.
metadata:
  type: project
---

**Superseded history:** before 2026-09-28, `rules_path` and `state_machine_path` were both live
at the same time. `feature/rules_to_state_machine_consolidation` (reviewed 2026-09-28) deletes
`rules_path`, `ProjectConfig.rules`, `LogicRulesAsset`, `LoadedRules`,
`message_interpreter_system` and `Action::EnterState`. Once that branch is on `integration`, do not
suggest rules.ron as a logic home. A stale `rules_path:` is a `deny_unknown_fields` parse error
(the runtime hangs on the loading screen; that is logged as a backlog bug).

**Why:** Frank decided on one deliberate breaking change, with no bridge.
Plan: `planning/features/rules_to_state_machine_consolidation.md`.

**Post-consolidation facts to apply:**
- `StateMachineAsset.initial_state`, `states` and `transitions` are all `#[serde(default)]`. A flat
  file with only `global_on` is valid. `validate()` now runs on the project FSM, on behaviors at
  runtime (`entity_spawner`) and in the CLI.
- **The `on:` overload is real:** `FsmState.on` is `Vec<FsmEventBinding>`, `FsmTransition.on` is
  `String`, and `FsmEventBinding` uses `event:`. Every mis-authoring is a loud parse error
  (`deny_unknown_fields`).
- **`FsmTransition` has NO `do_actions`.** Any doc that says to "put the action on the transition"
  is wrong. The actions belong in the destination state's `entry_actions` or the source state's
  in-state `on:` binding. docs/30 got this wrong in the 2026-09-28 review.
- **Initial-state entry_actions asymmetry:** the project FSM never fires `initial_state`'s
  `entry_actions` (project_loader only sets `LogicState`), but behavior FSMs do
  (`entity_spawner.rs` `resolve_pending_behaviors_system`). Any "gotcha" doc must say which of
  the two it means.
- The state change recipe is `EmitEvent("x")` + `( from?, on: "x", to: "S" )`. The transition
  fires on the next frame, because the executor emits the event after the interpreter has run.
- Designer-facing message strings still say "rules.ron" as of the review:
  `validate.rs` (unreachable_trigger message), `scene_loader.rs` ActionBar dup-key warn, and
  `.claude/agents/ron-gameplay-scripter.md`, which still offers `logic/rules.ron` and fake
  `on_enter`/`on_exit`/`when` fields.

**How to apply:** when a feature adds logic hooks, check only the `state_machine.ron`/behavior
paths. Related: [[validate-cross-file-blind-spots]], [[intent-event-layer-pattern]].

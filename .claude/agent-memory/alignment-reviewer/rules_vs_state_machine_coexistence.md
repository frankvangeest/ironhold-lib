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

- **Tripwire trap (2026-10-23, feature/leftover_rules_ron_check):** `ironhold_core` test
  `assets_schema_version_regression::regression_no_rules_ron_files_remain` scans
  `assets/projects/` AND `crates/ironhold_cli/tests/fixtures/` and fails on ANY `rules.ron`. Any
  CLI fixture that needs a real `logic/rules.ron` (e.g. the `leftover_rules_ron` strict check)
  must be allowlisted there or the file created at test time; CLI-only test runs won't catch it.
- Migration hint: rules with a `when:` guard go into a state's `on:`, not `global_on`. A message
  that says only "move into global_on" oversimplifies.

**How to apply:** when a feature adds logic hooks, check only the `state_machine.ron`/behavior
paths. Related: [[validate-cross-file-blind-spots]], [[intent-event-layer-pattern]].

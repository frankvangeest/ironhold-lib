---
name: fsm-designer-traps
description: Designer-facing traps in logic/state_machine.ron (StateMachineAsset) that matter more once rules.ron is removed - `on` means 3 things, initial_state entry_actions never run at boot, first-match-only transitions, EmitEvent missing from docs/20 action table
metadata:
  type: project
---

Verified 2026-09-23 at `1372cf4` during the plan review of
`planning/features/rules_to_state_machine_consolidation.md` (rules.ron removal, all logic onto
state_machine.ron):

- **`on` is overloaded 3 ways in one file:** `states[].on: [ ... ]` is a LIST of bindings;
  `transitions[].on: "event"` is an event STRING; bindings themselves use `event:` (not `on:`).
  Old rules.ron used `on:` for the event string, so a migrating designer's muscle memory writes
  `( on: "x", do_actions: [...] )` inside `global_on` -> parse error for the whole file.
- **`initial_state`'s `entry_actions` do NOT run at boot** (only a transition runs entry
  actions). Documented only as an inline snippet comment in docs/30 (~line 360). This is the cliff
  in the "start flat with global_on, add states later" progression.
- **Per-event order in `fsm_interpreter_system`:** global_on -> current state's `on:` -> first
  matching transition only (exit of old state -> entry of new). Multiple matching transitions:
  only the first in file order fires. rules.ron let every matching rule fire. Not documented in
  docs/30.
- **`EmitEvent(name)` has no row in docs/20's Action table** (EmitEventAfterDelay does). It is
  the replacement for `EnterState` from UI/dialogue/behavior `do_actions` (emit event + add a
  transition on it).
- Runtime load-failure message for state_machine.ron (project_loader.rs ~194) says "every state
  transition in this file is now inactive" - wrong framing for a flat global_on-only file.

**How to apply:** any change to logic authoring docs or the FSM schema should be checked
against these five; they are the places a former rules.ron author gets stuck. Related:
[[ron-parse-failure-diagnostics]], [[docs-lag-the-action-schema]],
[[ron-comments-cite-dev-paths]].

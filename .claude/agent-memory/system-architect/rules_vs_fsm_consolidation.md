---
name: rules-vs-fsm-consolidation
description: Open question (2026-09-23), Frank to decide: keep rules.ron alongside state_machine.ron, or consolidate on the FSM. My staged-consolidation recommendation, and the facts it rests on
metadata:
  type: project
---

On 2026-09-23 I recommended **consolidating project logic on `StateMachineAsset`, in stages**.
Frank has not decided yet. Full write-up: `planning/investigations/rules_vs_state_machine_architecture.md`.

**Why:** facts checked at `f64a970`:
- rules.ron is a strict subset of the FSM. A rule with no `when:` = `global_on`. A rule with
  `when: S` = an `on:` binding in state S. `EnterState` = a transition without the entry/exit hooks.
- None of the 11 live rules-only projects uses `when:` or `EnterState`. The only `when:` user is
  the dead `3rd_person_game_demo/logic/rules.ron`.
- Per-entity behaviors already use `StateMachineAsset`, so rules.ron is the only logic file that
  is not an FSM.
- Coexistence has caused a string of confusion bugs: the false warn, the "replaces" docs,
  dead files that look live, and `query`/`stats` counting dead files.
- Hazards when both are set:
  - both interpreters fire on the same event
  - rules check the whole frame against the start-of-frame `LogicState`, while the FSM changes
    state per event
  - `EnterState` from rules skips the FSM's exit/entry actions
  - nothing checks that `when:` names are real FSM states

**The key move:** make `StateMachineAsset.transitions`, `initial_state`, and `states` defaultable.
That is additive and non-breaking. A flat FSM file then becomes as short as rules.ron, which
removes the only real reason to keep rules.ron.

**The step that gets most of the benefit for the least churn:** at load time, convert
`LoadedRules` into the FSM, then delete `message_interpreter_system`. Rules-derived bindings go
*before* the file's own `global_on`, to keep today's order. Existing RON keeps working.

**How to apply:** if a future review touches logic-file loading, the CLI rules branches, or adds a
logic feature, check whether Frank has decided. Do not build new rules.ron-only features, and do
not build duplicated rules/FSM features, before that. Related: [[capability_patterns]],
[[cli-runtime-mirror-check-pairs]], [[cli-validate-coverage-model]].

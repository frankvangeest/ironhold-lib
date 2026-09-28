---
name: fsm-designer-traps
description: Designer-facing traps in logic/state_machine.ron after rules.ron removal - `on` means 3 things, initial_state entry_actions skip boot, first-match transitions, transitions carry NO actions, struct-variant actions written newtype-style in README/STATUS
metadata:
  type: project
---

Verified 2026-09-23 (plan review) and re-checked 2026-09-28 on
`feature/rules_to_state_machine_consolidation` (rules.ron removed, all logic on state_machine.ron):

- **`on` is overloaded 3 ways:** `states[].on: [ ... ]` = LIST of bindings; `transitions[].on: "event"`
  = STRING; bindings use `event:`. Now documented (docs/20 "Removed: rules.ron" + docs/30), but the
  WRONG/RIGHT snippet only covers the global_on case, not the state/transition cases.
- **`initial_state` entry_actions never run at boot.** Now has its own docs/30 heading; still missing
  from docs/20's initial_state table row and README.
- **Eval order:** global_on -> current state's on: -> first matching transition in file order. Self
  transitions (any-state `from` omitted, to == current) DO re-run exit+entry (fsm_interpreter.rs).
- **`FsmTransition` has only from/on/to - no actions field.** Docs that say "put the LoadScene on the
  transition" are wrong; it goes in the destination state's entry_actions.
- **`FsmEventBinding` (event, do_actions - do_actions REQUIRED) had no field table in docs/20.**
- **`PlayMusicLoop`/`PlaySound`/`Spawn` are struct variants** -> `PlayMusicLoop(key: "x")`. README
  action table and STATUS example wrote `PlayMusicLoop("x")`, `PlaySound("x")`, `Spawn { ... }` (braces
  are not RON) - copy-paste parse failures.
- **Leftover logic/rules.ron is silently ignored** - validate only warns (strict) for an unset
  state_machine_path with a state_machine.ron on disk, nothing for a rules.ron file.
- Stale old-syntax snippets survived the migration in docs/20 (intent.slot `when:` examples ~1159,
  decal example ~1815, co-op join note ~3143). Grep `\(\s*on:\s*"[^"]*",\s*do_actions` and `when:\s*"`
  across docs/README on any logic-doc change.

**How to apply:** any change to logic authoring docs or the FSM schema should be checked against
these. Related: [[ron-parse-failure-diagnostics]], [[docs-lag-the-action-schema]],
[[ron-enum-double-paren]].

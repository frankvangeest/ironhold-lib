---
name: rules-vs-state-machine-coexistence
description: rules_path and state_machine_path are both live when both are set. The project_loader.rs warn was fixed in feature/fix_stale_logic_path_warning (2026-09-22); docs/00 and docs/30 still say FSM "replaces" rules.ron
metadata:
  type: project
---

`rules_path` and `state_machine_path` are **independently live**: if both are set, both run.

**Why:** found by reading the code during the `feature/configurable_logic_paths` review
(2026-09-06). `check_project_loaded` builds `rules_handle` (project_loader.rs:~49-53) without
checking for a state machine, and inserts `LoadedRules` whether or not the FSM resolved.
`message_interpreter_system` reads `LoadedRules` unconditionally and runs alongside
`fsm_interpreter_system`. Neither one turns the other off.

**Status (2026-09-22):** `feature/fix_stale_logic_path_warning` changed the old false warn text
("rules.ron is NOT loaded...") to say correctly that both files are live when both are set. It
changed wording only. `docs/20_data_formats.md` was fixed earlier. Places that still suggest
FSM replaces rules.ron, as of that review:
- `docs/00_overview.md`:~121: "`state_machine_path` instead of `rules_path`"
- `docs/30_runtime_events_and_logic.md`:~45: "Replaces `rules.ron` for FSM projects"
- `planning/backlog.md`: the v2→v3 migration-guide item mentions "the warning to expect if both
  files coexist"
- The debug-detective and system-architect memory files still quote the old warn text

**How to apply:** don't copy the old exclusivity idea into a CLI check, and don't treat the two
fields as mutually exclusive. `resolve_logic_files` already treats them as independently live, and
the `valid_ui_trigger` fixture is the regression coverage for that. Making them truly exclusive
would break any project that uses both, so that is Frank's decision, not something a review should
decide. Related: [[validate-cross-file-blind-spots]].

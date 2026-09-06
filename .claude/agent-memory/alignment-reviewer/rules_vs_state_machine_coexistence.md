---
name: rules-vs-state-machine-coexistence
description: project_loader.rs's "rules.ron is NOT loaded when state_machine_path is present" warn is factually wrong — both are loaded and both interpreters run; docs and backlog repeat the same false claim
metadata:
  type: project
---

`rules_path` and `state_machine_path` are **independently live** — setting both means both run.

**Why:** established reading the code during the `feature/configurable_logic_paths` review
(2026-09-06). `check_project_loaded` builds `rules_handle` (project_loader.rs:~49-53) with no
state-machine condition, and inserts `LoadedRules` from it regardless of whether `fsm` resolved
(~249-254). `message_interpreter_system` reads `LoadedRules` unconditionally
(message_interpreter.rs:13) and runs alongside `fsm_interpreter_system` — neither suppresses the
other. But project_loader.rs:~59-64 emits a `warn!` claiming *"rules.ron is NOT loaded when
state_machine_path is present — remove rules_path to silence this"*, which contradicts its own
code. The same false claim is repeated in `docs/20_data_formats.md` (the `state_machine_path` table
row "use instead of `rules_path`", and the `StateMachineAsset` section "Replaces `rules.ron` for
FSM-based projects") and in `planning/backlog.md`'s v2→v3 migration-guide item ("rename
`rules_path` → `state_machine_path` ... and the warning to expect if both files coexist").

**How to apply:** never mirror that warn in a CLI check or treat the two fields as mutually
exclusive — `resolve_logic_files` correctly treats them as independently live, and the
`valid_ui_trigger` fixture (sets both, passes `unreachable_trigger` and `--strict orphan_rule` at
exit 0) is the regression coverage for it. If a future change is asked to "make validate match the
warning," push back: the warn is what's wrong, and a designer who trusts it and deletes their
`rules_path` silently loses working rules. Fixing it is either a warn-text correction in
`ironhold_core` (cheap, no behavior change) or an actual exclusivity implementation (a breaking
change for any project relying on coexistence) — that is a Frank decision, not a review call.
Related: [[validate-cross-file-blind-spots]].

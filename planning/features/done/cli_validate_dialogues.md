# Feature: `ironhold_cli validate` parses `dialogues/*.dialogue.ron`

_Status: Done_
_Planned at: `cbe2f2a` (2026-09-04)_
_Completed: `63b31e6` (2026-09-04)_

## What

`ironhold_cli validate` now parses `dialogues/*.dialogue.ron` files the same way it already
parses `rules.ron`, `state_machine.ron`, and `behaviors/*.behavior.ron` — schema/parse errors in a
dialogue file are now caught at design time, and a dialogue choice's `do_actions` participates in
every existing cross-file reference check (missing effect/audio/prefab/item keys, missing scene
paths, etc.) the same way a rule's `do_actions` does. Also added: on-disk existence checks for
`PrefabDef.dialogue` and `Action::StartDialogue`'s `dialogue_path`.

## Why

`dialogues/*.dialogue.ron` was the only `Action`-bearing authoring surface in the project with
neither CLI coverage nor a runtime diagnostic. Found independently by 3 of 4 reviewers during the
previous `action_deny_unknown_fields` feature's post-implementation review, then logged to
`planning/backlog.md` ▸ Designer Experience.

## Approach

- `do_validate` (`crates/ironhold_cli/src/commands/validate.rs`): add a `glob_dir(project_dir,
  "dialogues", ".dialogue.ron")` + `parse_file::<DialogueDef>` pass, mirroring the existing
  `behaviors` loop exactly.
- `collect_actions`: extended signature to take `dialogues: &[(String, DialogueDef)]`, walking
  `nodes[].choices[].do_actions` — no recursion needed (no `Action` variant nests another).
- `cross_file_checks`/`strict_checks` needed no changes — both are already source-agnostic,
  matching on the `Action` variant and threading `source` through into the error, so a
  dialogue-sourced error is attributed correctly for free.

## Tasks
- [x] `dialogues/*.dialogue.ron` glob + parse pass in `do_validate`
- [x] `collect_actions` extended to walk dialogue `do_actions`
- [x] `PrefabDef.dialogue` on-disk existence check (found by all 3 reviewers)
- [x] `Action::StartDialogue.dialogue_path` on-disk existence check (found by all 3 reviewers)
- [x] 4 new fixture tests (parse error, cross-file reference, both new path checks)
- [x] Strengthened the reference test to assert the correct source-file attribution
- [x] Docs: `docs/60_contributing.md` (checks list, test enumeration), `docs/20_data_formats.md`
      (project layout tree)
- [x] Full `ironhold_core` suite + `ironhold_cli` suite green
- [x] Live CLI output demonstrated in place of a browser playtest (this is a CLI-only change, no
      engine/WASM code touched)

## Deferred (logged to `planning/claude_suggestions.md`, not this branch's scope)
- `jump_to` target / duplicate node `id` checks — the runtime already treats both as fatal
  (`capabilities/dialogue.rs`'s `warn!` closes the conversation mid-flow on a bad `jump_to`)
- `DialogueCondition::StatAtLeast`'s unchecked `stat_key` — same shape as the existing
  `ApplyModifier`/merchant `currency_stat` checks
- `query.rs`/`stats.rs` are now dialogue-blind relative to `validate.rs`

## Open questions
- None.

## Acceptance criteria
- Given a `.dialogue.ron` file with a schema/parse error, when `ironhold_cli validate` runs, then
  the error is reported with file, line, and column — same as every other RON file type.
- Given a dialogue choice's `do_actions` referencing a missing effect/audio/prefab/item/scene key,
  when `ironhold_cli validate` runs, then it's reported the same as an equivalent rule would be.
- Given a `PrefabDef.dialogue` or `Action::StartDialogue.dialogue_path` pointing at a file that
  doesn't exist, when `ironhold_cli validate` runs, then it's reported as `missing_file`.
- Given every currently-shipped `assets/projects/*` project, when validated under this change, then
  all still validate clean (no false positives).

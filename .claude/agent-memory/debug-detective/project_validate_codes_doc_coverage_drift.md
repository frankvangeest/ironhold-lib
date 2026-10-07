---
name: validate-codes-doc-coverage-drift
description: docs/15_authoring_tools.md is declared the canonical list of validate checks, but at creation 16 always-on error codes had no row; enumerate codes from validate.rs to audit it
metadata:
  type: project
---

`docs/15_authoring_tools.md` (docs audience split, 2026-10) is declared the single canonical list of
`ironhold validate` checks, one row per code. When it was created, 16 `CrossFileError` codes in
`crates/ironhold_cli/src/commands/validate.rs` had no row (e.g. `reserved_camera_mode_key`,
`invalid_gamepad_key`, `flycam_model_never_renders`, `misplaced_new_id_token`,
`unsupported_join_prefab`). The old `docs/60_contributing.md` "Checks performed" list had the same
gap, so this is long-standing drift: new checks land with tests but no doc row.

Severity also drifts: `non_ascii_char_in_text` is a `StrictWarning` (inside `strict_checks`, which
starts around the `fn strict_checks` line), but the first draft of docs/15 filed it as always-on.

**Why:** a designer looking up a code from `--json` output finds nothing, or runs plain `validate`
expecting a check that only `--strict` runs.

**How to apply:** when reviewing a validate change or docs/15, run
`grep -noE '(kind|error_type): "[a-z_]+"' validate.rs` and diff the codes against docs/15. Classify
severity by the enclosing struct (`StrictWarning` vs `CrossFileError`), not by guessing. UI layout
codes (`inert_*`, `percent_under_auto`, ...) live in `ironhold_core/src/schema/scene_v2.rs`, not
validate.rs. Related: [[cli-validate-never-calls-schema-validate]], [[collect-actions-skips-actionbar-slots]].

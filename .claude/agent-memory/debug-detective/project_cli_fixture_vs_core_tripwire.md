---
name: cli-fixture-vs-core-tripwire
description: ironhold_core's assets_schema_version_regression scans crates/ironhold_cli/tests/fixtures too, so a CLI fixture holding a "forbidden" file (rules.ron) fails a core test no CLI run catches
metadata:
  type: project
---

`regression_no_rules_ron_files_remain` (crates/ironhold_core/tests/assets_schema_version_regression.rs) walks BOTH
`assets/projects/` and `crates/ironhold_cli/tests/fixtures/` and fails on any file named exactly `rules.ron`. A CLI
feature that adds a fixture to exercise "leftover rules.ron" detection (seen on feature/leftover_rules_ron_check,
2026-10-23) passes `cargo test -p ironhold_cli` but breaks the core suite.

**Why:** the tripwire lives in a different crate than the fixture, so the implementer's own test run never sees it.

**How to apply:** when reviewing a new CLI fixture that deliberately contains a deprecated/forbidden file, grep
ironhold_core/tests for repo-wide tripwires on that filename. Fix = an explicit allowlist entry in the tripwire, or
build the file in a temp dir at test time. Same review: a multi-line string literal whose `\` line continuations got
eaten shows up as runs of ~20 spaces inside the message (check with `cat -A`).

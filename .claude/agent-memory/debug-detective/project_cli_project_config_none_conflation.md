---
name: cli-project-config-none-conflation
description: validate's project_config None means BOTH "no .project.ron" and "it failed to parse"; a check that falls back to a config field's default then cascades a misleading second error
metadata:
  type: project
---

In `ironhold_cli` validate, `project_config` comes from `find_project_ron(..).and_then(try_parse)`, so a
`.project.ron` that exists but fails to parse (any typo; `ProjectConfig` is `deny_unknown_fields`) is
indistinguishable from "no project file". Cross-file and strict checks still run after a parse failure.

**Why:** found reviewing item system S2b (2026-11-15): `inventory_bag_issues` does
`project_config.map(..).unwrap_or_default()`, so `player_inventory: (max_slot: 12)` (typo) plus a 4x3 panel
yields the parse error AND "bag holds 20 slots (player_inventory is not set ...)", which tells the designer
the opposite of what they wrote.

**How to apply:** when reviewing any new validate check that reads a `ProjectConfig` field with a default,
ask what it does when the config is `None` because of a parse failure; suggest skipping the check when
`find_project_ron(project_dir).is_some()` but the config is `None`. See [[cli-plain-ron-from-str-implicit-some]]
for the other project.ron parse trap.

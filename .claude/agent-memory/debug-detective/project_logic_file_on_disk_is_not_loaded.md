---
name: logic-file-on-disk-is-not-loaded
description: A logic/*.ron file existing on disk does NOT mean the runtime loads it — paths come from ProjectConfig.rules_path/state_machine_path, plus an inline config.rules fallback; validate.rs assumes the convention paths
metadata:
  type: project
---

`ironhold_cli`'s `validate.rs`/`utils.rs` read logic from the **hardcoded convention paths**
`logic/rules.ron` and `logic/state_machine.ron`. The runtime does not:
`runtime/scene_manager/project_loader.rs` loads whatever `ProjectConfig.rules_path` /
`state_machine_path` name, and when neither is set it falls back to the **inline**
`config.rules: Vec<LogicRule>` (V1 style, `project_loader.rs` ~line 243 / 111).

Three divergences follow, all verified empirically 2026-09-04 by running `validate` on
hand-built fixtures:

1. **Inline `config.rules`** — a V1 project authoring rules in the `.project.ron` has zero
   files under `logic/`. Any validate check that reads `logic/rules.ron` sees nothing.
2. **Custom filename** — `rules_path: "logic/menu_rules.ron"` is legal and loaded; the
   convention-path read finds nothing.
3. **Dead file on disk** — `3rd_person_game_demo` and `terrain_demo` both ship a
   `logic/rules.ron` while their `.project.ron` sets only `state_machine_path`. That file is
   **never loaded** (and produces no warning — `project_loader.rs`'s "rules.ron is NOT loaded
   when state_machine_path is present" warn only fires when *both* paths are set). A validator
   reading it will treat dead rules as live.

**Why:** matters for any cross-file check whose verdict depends on "what logic exists" —
reachability, orphan detection, action collection. Directions 1 and 2 give hard exit-1 false
positives; direction 3 gives silent false negatives.

**How to apply:** when adding or reviewing a `validate.rs` check that consults rules/FSM
content, resolve the paths from `ProjectConfig` (falling back to the convention path only when
unset) and union in `config.rules` — do not read `logic/rules.ron` unconditionally. Related:
[[validate-reference-checks-token-blind]].

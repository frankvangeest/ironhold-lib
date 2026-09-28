---
name: project-cli-plain-ron-from-str-implicit-some
description: Any CLI/tool code that parses ProjectConfig (or any schema with Option fields) via plain ron::de::from_str instead of utils::ron_from_str fails on EVERY shipped project, because they all write bare strings into Option<String> fields relying on IMPLICIT_SOME
metadata:
  type: project
---

Every shipped `.project.ron` writes `state_machine_path: "logic/state_machine.ron"` (bare string into
`Option<String>`), which only parses with `Extensions::IMPLICIT_SOME`. The runtime loader and the CLI's
`utils::ron_from_str`/`silent_parse` enable it; plain `ron::de::from_str` does not.

Concrete instance (2026-09-28, feature/rules_to_state_machine_consolidation review): a new
"project.ron exists but failed to parse" stderr warning in `utils::resolve_catalog_paths` used plain
`ron::de::from_str::<ProjectConfig>` and fired on all 15 shipped projects on every `stats`/`query`
run — 100% false positive, proven by running the branch's debug `ironhold.exe` from scratchpad.

**Why:** the extension is invisible in the RON text; a parse that "obviously" should succeed silently
fails and the error branch looks like a legit diagnostic.

**How to apply:** when reviewing any new `ron::` parse call in CLI/tools/tests, grep that it goes
through `ron_from_str` (or `Options::default().with_default_extension(IMPLICIT_SOME)`). A quick
liveness check: run the command against `assets/projects/quick_scene` — a valid project must produce
no warning. Related: [[project_stale_cli_binary_as_prefix_oracle]], [[project_shared_target_binary_clobbering]].

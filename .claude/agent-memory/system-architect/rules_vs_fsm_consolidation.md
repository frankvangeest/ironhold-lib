---
name: rules-vs-fsm-consolidation
description: DECIDED 2026-09-23 — rules.ron is being removed outright (one breaking change, no bridge); plan at planning/features/rules_to_state_machine_consolidation.md; the non-obvious traps the plan rests on
metadata:
  type: project
---

**Decision (Frank, 2026-09-23):** remove `rules_path`/inline `rules:`/`message_interpreter_system`
as ONE deliberate breaking change. Frank explicitly rejected a load-time rules→FSM translation
bridge ("do it right the first time"). Reasons: he is pre-1.0 with no stable release, he is the
only author, and the only external consumer (`Ironhold-fps-demo`, pinned via a `pkg/` commit)
already uses `state_machine_path` only. Plan: `planning/features/rules_to_state_machine_consolidation.md`
(Draft, planned `59f10e9`). Background: `planning/investigations/rules_vs_state_machine_architecture.md`.

**Why:** rules.ron is a strict subset of the FSM. None of the live rules files (10 files,
~139 rules) or the 38 CLI fixture rules files (~91 rules) uses `when:` or `EnterState`. They are
all flat `global_on` lists.

**Non-obvious facts the plan depends on (re-verify before acting):**
- `message_interpreter_system` is the `.before()` ordering ANCHOR for 9 registrations (8 in
  lib.rs, 1 in action_bar.rs), not only a chain member. Each must be re-anchored to
  `fsm_interpreter_system`.
- Default `initial_state` to `""` so migrated projects keep `LogicState==""`. That value is
  exposed via the `#debug-state` DOM JSON to test_web.py.
- No `ProjectConfig` schema_version bump. Its `deny_unknown_fields` already turns a leftover
  `rules_path:` into a hard parse error that names the field.
- Migration = a comment-preserving text transform plus a temporary parsed-equivalence test
  (`Action: PartialEq`). Parse-and-reserialize would destroy ~100 designer comments.
- ~32 CLI fixtures have no `.project.ron`. They rely on validate's convention fallback.
- The rules_path-semantics fixtures are the only coverage of the configured-path code. Port them
  to state_machine_path; do not delete them.
- `Action::EnterState` removal is the plan's default. Frank may veto it.

**How to apply:** when reviewing the implementation, check the re-anchoring, the tripwire test
(no `rules.ron` anywhere), and that `query`/`stats` now resolve `state_machine_path`. Do not
accept a bridge or a `migrate-rules` subcommand, which Frank rejected. After it ships, update
[[capability_patterns]] and [[cli-validate-coverage-model]] (their rules.ron branches go away).
Related: [[cli-runtime-mirror-check-pairs]], [[schedule_ordering_mechanism]].

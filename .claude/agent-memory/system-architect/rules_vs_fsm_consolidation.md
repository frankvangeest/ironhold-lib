---
name: rules-vs-fsm-consolidation
description: DECIDED 2026-09-23 — rules.ron is being removed outright (one breaking change, no bridge); plan at planning/features/rules_to_state_machine_consolidation.md; the non-obvious traps the plan rests on, plus the gaps an adversarial plan-review found
metadata:
  type: project
---

**Decision (Frank, 2026-09-23):** remove `rules_path`/inline `rules:`/`message_interpreter_system`
as ONE deliberate breaking change. Frank explicitly rejected a load-time rules→FSM translation
bridge ("do it right the first time"). Reasons: he is pre-1.0 with no stable release, he is the
only author, and the only external consumer (`Ironhold-fps-demo`, sibling checkout at
`../Ironhold-fps-demo`, pinned via a `pkg/` commit) uses `state_machine_path` only. Verified
2026-09-23: it has no `rules:`/`EnterState`/`when:`. Plan: `planning/features/rules_to_state_machine_consolidation.md`
(Draft, planned `59f10e9`). Background: `planning/investigations/rules_vs_state_machine_architecture.md`.

**Why:** rules.ron is a strict subset of the FSM. None of the live rules files (10 files,
~139 rules) or the 38 CLI fixture rules files (~91 rules) uses `when:` or `EnterState`. They are
all flat `global_on` lists.

**Facts verified in the 2026-09-23 plan-review (still re-check before acting):**
- Re-anchoring claim is CORRECT. There are exactly 9 `.before(message_interpreter_system)` sites
  (8 in lib.rs, 1 in action_bar.rs), zero `.after(...)`, zero test references. `before(fsm)` is
  equivalent via the chain.
- `fsm_interpreter_system` early-returns BEFORE reading its MessageReaders when
  `LoadedStateMachine` is None. The rules interpreter always drained them. Result: on a migrated
  project, the load-frame `scene.requested:<initial>` now reliably reaches the FSM. No live content
  binds `scene.requested`, so nothing observable changes, but the equivalence proof doesn't state it.
- **`StateMachineAsset::validate()` is called in exactly ONE place:** `project_loader.rs`, and
  only as a logged `error!`. It is never called for behavior files (runtime or CLI), and the CLI
  never calls it at all. Loosening `initial_state` to `#[serde(default)]` therefore turns a
  forgotten `initial_state:` in any of the 21 behavior files from a loud parse error into a silent
  "entity stuck in state ''". The plan's "validate() still guards it" claim holds at no tool
  surface. It needs validate() wired into the CLI and into `resolve_pending_behaviors_system`.
- A stale `rules_path:` in ProjectConfig: the runtime logs one bevy_asset error, then hangs
  forever in LoadingProject (there is no Failed arm for the ProjectConfig handle). CLI `validate`
  fails loudly. **`query`/`stats` are silent:** `utils.rs::resolve_catalog_paths` uses
  `silent_parse` on `.project.ron`, so they fall back to convention paths with no diagnostic.
- Migration-script traps: 2 fixtures (`unset_rules_path_with_convention_file`,
  `state_machine_only_ignores_dead_rules_ron`) already have a `state_machine.ron`, so a naive write
  clobbers it. `bad_rules_parse_no_cascade` is unparseable on purpose. `rules_path_case_mismatch`
  uses `"Logic/Rules.ron"`, so a literal line flip misses it. camera_modes has parens inside a
  `Log` string. All 48 files are CRLF in the working tree (autocrlf=true).
- Baseline trap: `query actions` hardcodes `logic/rules.ron`, and `--strict` emits
  `unset_logic_path_with_convention_file`. So a commit-0 baseline for 3rd_person_game_demo and
  terrain_demo will legitimately differ after commit 1. The revised plan captures the baseline
  after commit 2 (not just after commit 1), because commit 2's new validate() wiring can also
  legitimately add diagnostics. Rebuild tools/bin/ironhold before each capture.
- Plan revised 2026-09-23 to fold in all of the above plus ux-gamedesigner-reviewer's findings
  (README, EmitEvent recipe in designer docs, `on:` overload, initial_state entry_actions do not
  run at boot). Status is now Ready. The only open Frank question is the EnterState veto.

**How to apply:** when reviewing the implementation, check the re-anchoring, the tripwire test
(no `rules.ron` anywhere), that `query`/`stats` now resolve `state_machine_path` AND warn on an
unparseable `.project.ron`, and that `StateMachineAsset::validate()` is wired in. Do not accept a
bridge or a `migrate-rules` subcommand, which Frank rejected. A permanent CLI diagnostic for a
leftover `logic/rules.ron` is not a bridge. After it ships, update [[capability_patterns]] and
[[cli-validate-coverage-model]] (their rules.ron branches go away). Related:
[[cli-runtime-mirror-check-pairs]], [[schedule_ordering_mechanism]].

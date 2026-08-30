---
name: stale-feature-base-branch-hazard
description: Feature branches are cut from main, but schema changes land on integration first — always diff the reviewed branch's Action/schema struct shape against the primary checkout before signing off
metadata:
  type: project
---

The workflow cuts every feature branch from `main` (`git worktree add ../ironhold-lib-{slug} -b
feature/{slug} main`), but merged features sit on `integration` for a whole batch before `main`
fast-forwards. So a feature branch can be reviewed, tested green, and still not compile once merged.

**Why:** enum-struct-variant literals. When an earlier batch adds a field to an existing variant
(e.g. `Action::Spawn.at_entity`), every `Action::Spawn { ... }` literal in `tests/` gains
`at_entity: None` on `integration`. A branch cut from `main` writes new test functions with the
old 5-field literal; git merges those new functions in cleanly (no textual conflict — they are
brand-new hunks) and the test crate then fails to compile. Feature-branch CI cannot see this.
Confirmed concretely on `feature/monotonic-entity-id` vs `integration` (2026-08-29).

**How to apply:** when reviewing a feature-branch worktree, grep the *primary* checkout
(`C:\workspace\frank\projects\Ironhold\ironhold-lib`) for the same schema type before signing off.
If the field sets differ, call it out and list every conflict site — typically:
`schema/*.rs` doc comment + variant, the `action_executor.rs` match arm,
`message_interpreter.rs::rewrite_self`/`rewrite_target`, `dialogue.rs::substitute_self_in_action`,
`capabilities/action_bar.rs::action_needs_target`, the matching `tests/*_tests.rs` literals,
`docs/20_data_formats.md`, and `docs/30_runtime_events_and_logic.md`.

The doc-comment conflicts matter as much as the code: two branches both rewriting the same
`Action` doc block is how a designer-facing bullet (e.g. the `at_entity` line, or the `{new_id}`
paragraph) gets silently dropped during hand resolution.

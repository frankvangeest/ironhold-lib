---
name: project-self-target-substitution-coverage
description: "{self}/{target} substitution has four separate hand-written match arms (rewrite_self, rewrite_target, dialogue's substitute_self_in_action, action_bar's action_needs_target); {self} only works in per-entity behavior files, and action_needs_target still only sees Spawn's at_entity"
metadata:
  type: project
---

There is no single substitution engine. Four hand-written match arms must be kept in sync by hand:

- `rewrite_self` (`runtime/scene_manager/message_interpreter.rs`) — called **only** by
  `entity_fsm_interpreter_system`. So `{self}` works in per-entity behavior files and *not* in a
  global `rules.ron` / `state_machine.ron`, despite docs describing it generically. Handles
  `Action::Spawn`'s `id`, `spawn_point`, and `at_entity`.
- `rewrite_target` (same file) — called by all three interpreter systems. Same three `Spawn` fields.
- `substitute_self_in_action` (`capabilities/dialogue.rs`) — the only transform applied to dialogue
  `do_actions`. **It now HAS an `Action::Spawn` arm** covering `id`/`spawn_point`/`at_entity` (added
  during `monster_corpse_loot.md` v2 — the older note that it omitted `Spawn` entirely is stale).
  Dialogue still applies no `{target}` pass at all.
- `action_needs_target` (`capabilities/action_bar.rs:380`) — the *gate* deciding whether an action-bar
  slot requires a target before firing. Its `Action::Spawn` arm inspects **only `at_entity`**, so a
  slot authored as `Spawn(spawn_point: "{target}_spawn")` or `Spawn(id: "{target}_x")` is treated as
  target-free, fires with no target, and leaves the token literal.

Net effect: a `Spawn` token resolves in a behavior file and in a dialogue choice, silently stays a
literal in a global rule (`{self}` only), and is mis-gated on the action bar unless it is in
`at_entity`. Nothing warns in any of those cases.

**Why:** each arm was extended independently as actions were added; the doc comments on `Action`
describe substitution as a property of the *field*, which reads as universal but is actually a
property of the *call path*.

**How to apply:** when a review adds or documents token support on an action field, check all four
functions and which interpreter systems call them. When a token silently fails to resolve the
resulting string keeps its literal braces; a `warn!` on a residual `{` in a consumed value is the
general fix and does not exist anywhere today. This also makes any raw-string CLI reference check on
such a field a false-positive source — see
[[project_validate_reference_checks_token_blind]].

Related: [[project_spawn_id_single_namespace]], [[project_action_ron_typos_are_silent]].

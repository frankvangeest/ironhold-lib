---
name: project-self-target-substitution-coverage
description: "{self}/{target} substitution has three separate, non-identical implementations; dialogue's omits Action::Spawn entirely and never substitutes {target}, and {self} only works in per-entity behavior files"
metadata:
  type: project
---

There is no single substitution engine. Three hand-written match arms must be kept in sync by hand,
and they are **not** currently in sync:

- `rewrite_self` (`runtime/scene_manager/message_interpreter.rs`) — called **only** by
  `entity_fsm_interpreter_system`. So `{self}` works in per-entity behavior files and *not* in a
  global `rules.ron` / `state_machine.ron`, despite docs describing it generically.
- `rewrite_target` (same file) — called by all three interpreter systems. Handles `Action::Spawn`'s
  `id` and `spawn_point`.
- `substitute_self_in_action` (`capabilities/dialogue.rs`) — the only transform applied to dialogue
  `do_actions`. It has **no `Action::Spawn` arm** (falls through `other => other`), and dialogue
  applies no `{target}` pass at all.

Net effect: `Spawn(id: "{self}_x")` resolves in a behavior file, and silently stays a literal
`"{self}_x"` in both a global rule and a dialogue choice. Nothing warns.

**Why:** each arm was extended independently as actions were added; the doc comments on `Action`
describe substitution as a property of the *field*, which reads as universal but is actually a
property of the *call path*.

**How to apply:** when a review adds or documents token support on an action field, check all three
functions, and check which interpreter systems call them — a doc claim of "`{self}`/`{target}`
supported" is only true for the behavior-file path unless verified otherwise. When a token silently
fails to resolve, the resulting string keeps its literal braces; a `warn!` on a residual `{` in a
consumed value is the general fix and does not exist anywhere today.

Related: [[project_spawn_id_single_namespace]], [[project_action_ron_typos_are_silent]].

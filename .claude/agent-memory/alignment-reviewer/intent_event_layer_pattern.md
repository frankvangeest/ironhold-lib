---
name: intent-event-layer-pattern
description: The intent-event suppression mechanism that routes action-bar ability activation through the interpreter; the canonical example of a capability emitting intent + a flush-suppress fallback
metadata:
  type: project
---

The intent event layer (shipped ~2026-06-25, feature file `planning/features/intent_event_layer.md`) is the reference implementation of the intent-event pattern the alignment rubric describes. When reviewing future intent additions (`intent.interact:{id}`, `intent.attack:{id}` — both deferred to v2), check against this.

**The mechanism (three resources, two-phase dispatch):**
- `PendingIntentActions(HashMap<slot_key, Vec<Action>>)` and `HandledIntentSlots(HashSet<slot_key>)` live in `capabilities/action_bar.rs`.
- `action_bar_input_system` (runs `.before(message_interpreter_system)`): validates cooldown/cost/target, emits `GameEvent::Trigger("intent.slot.{n}:{player_id}")`, stores `{target}`-rewritten do_actions (+cost ModifyStat) in `PendingIntentActions`, also still emits `action_bar.activated:{n}`. It does NOT push to ActionQueue.
- All three interpreters take `mut handled_intents: ResMut<HandledIntentSlots>`. `match_rules` now returns bool; when an event starting `intent.slot.` matches a rule/binding/transition, the interpreter inserts the slot key into `HandledIntentSlots` via `intent_slot_key()` helper.
- `flush_pending_intent_system` (in lib.rs chain between `entity_fsm_interpreter_system` and `action_executor_system`): drains PendingIntentActions; for each slot NOT in HandledIntentSlots, pushes its actions to ActionQueue; then clears HandledIntentSlots.

**Why this is aligned (not a bypass):** the capability never pushes to ActionQueue. `flush_pending_intent_system` is effectively a fourth interpreter-tier system — it lives in the interpreter chain, runs after all rule matching, and is the only thing besides the three interpreters that pushes. The slot's do_actions are the designer-authored default path; a rule on `intent.slot.{n}:{id}` suppresses+replaces them. Zero migration cost.

**Footguns / known gaps to flag if extended:**
- `intent_slot_key()` only recognizes the `intent.slot.` prefix. A future `intent.interact:`/`intent.attack:` will need its own prefix handling AND its own pending/handled tracking, or a generalized key scheme. Do not assume one HashSet covers all intent namespaces.
- action_bar pending actions get `{target}` rewrite but never `{self}` rewrite — consistent with pre-feature action_bar behavior (it never did {self}); slot do_actions address the player by explicit ID or `{target}`. Not a regression, but if intents ever fire from per-entity sources, {self} will be needed.
- No shipped example project demonstrates an intent rule (demo task left unchecked in feature file). Only coverage is integration_tests.rs (~line 3872, rule-handled SetVariable case). WARNING-level: designers have docs but no copyable working project rule.
- `action_bar.activated:{n}` fires unconditionally even when the intent is later suppressed — documented as intentional (line 129 of docs/30). This is correct: it is a notification/telemetry event, not the action-commit path.

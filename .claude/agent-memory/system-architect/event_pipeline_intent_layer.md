---
name: event-pipeline-intent-layer
description: How ironhold's Message→Interpreter→Action→Result pipeline maps to intent/execution/result; the action_bar chokepoint that breaks interception; recommended minimal fix
metadata:
  type: project
---

The Message→Interpreter→Action→Executor pipeline IS an intent/execution/result implementation
**for events that flow through the interpreter**. Intent = UiEvent/InputActionMessage; Result =
GameEvent::Trigger("noun.verb:{id}"). The result-phase reactions in state_machine.ron
(action_bar.activated/on_cooldown/no_target) are already designer-authored — that part works.

**The one real gap: the action bar shortcuts the pipeline.** `action_bar_input_system` in
`capabilities/action_bar.rs` (lines ~82-152) does cooldown/cost/target validation in Rust and
pushes `slot.do_actions` DIRECTLY to ActionQueue (line ~133), THEN emits `action_bar.activated:{n}`
as an after-the-fact notification. So no designer rule can intercept between press and commit —
can't author "if silenced, cancel" or "if shield buff, convert to parry". This is the ONE capability
that violates the CLAUDE.md "only interpreters push to ActionQueue" rule.

**Why:** Frank is deciding whether to refine event architecture around intent/execution/result
before building combat (nameplate is queued first, then combat). Goal: non-programmer authors rich
RPG interactions entirely in RON.

**IMPLEMENTED (reviewed 2026-06-26):** The intent layer shipped along the recommended lines.
`action_bar_input_system` now emits `intent.slot.{key}:{player_id}`, parks slot do_actions in a
`PendingIntentActions` resource, and `flush_pending_intent_system` (new, chained between
`entity_fsm_interpreter_system` and `action_executor_system` in lib.rs) commits them ONLY if no
interpreter set the slot key in `HandledIntentSlots`. All three interpreters insert into the
HashSet on an `intent.slot.*` match (idempotent — multi-match is safe). Ordering is sound (single
`.chain()`, action_bar `.before(message_interpreter_system)`). WASM-clean, no schema change.

Two findings from that review — **BOTH FIXED as of 2026-07-15 re-read**: cooldown is now committed
inside `flush_pending_intent_system` (action_bar.rs ~207) on the `!handled` path only, and
`action_bar.activated:{key}` now fires from the flush on the committed path while `action_bar.pressed:{key}`
is the press-time event (~184). Leaving the original text below for history:
- ~~CRITICAL: cooldown leaks on suppression~~ (fixed)
- ~~MAJOR: `action_bar.activated:{key}` fires unconditionally~~ (fixed)
- Suppression is all-or-nothing per slot: "handled" = "at least one rule matched," and multiple rule
  sources (global + entity FSM) can each contribute actions to one intent. Document this for designers.

**Two durable action-bar facts relevant to per-player / split-screen (verified 2026-07-15):**
- **`action_bar_input_system` fires at most ONE slot per frame**: it does
  `DIGIT_KEYS.iter().find(just_pressed)` then `return` (action_bar.rs ~118-122) — no outer loop over
  slots. Fine for single-player, but two split-screen players pressing in the same 16ms frame drops
  one input. Any per-player action-bar work MUST restructure this to a per-slot loop, not just swap
  the target lookup.
- **Intent suppression is keyed on slot_key alone, player id is discarded.** `action_bar_input_system`
  emits `intent.slot.{key}:{player_id}` but `intent_slot_key()` (message_interpreter.rs ~83) splits on
  `:` and keeps only the key, so `HandledIntentSlots: HashSet<String>` and `PendingIntentActions`/
  `CooldownMap: HashMap<String,_>` are all scene-wide slot-key-only. This is the concrete reason
  composite `(player, slot_key)` keying is more than a one-liner — it touches the interpreter's
  intent-extraction + suppression path, not just the maps. Two bars sharing a slot key → shared
  cooldown + ambiguous fire + cross-player suppression. The per-player-targeting Phase 2 plan works
  around this by requiring disjoint keys across bars (relying on action_bar_custom_hotkeys) rather
  than fixing the keying. Cost cost gate reads the single global `LoadedStats` resource, never the
  per-entity `StatMap` component — per-player resource pools are unbuilt.

**How to apply (recommended direction, verified 2026-06-25):**
- Fix is "route action bar through interpreter," NOT "rebuild event system." Pipeline shape is right.
- Minimal intent layer: action bar emits `intent.{slot}` GameEvent (strings, NOT a typed enum — typed
  intents kill designer extensibility). Rule maps intent→actions. Cancellation = no rule for that
  intent while in a given LogicState (reuses existing `when:`/LogicState gate, zero new primitives).
- Keep `do_actions` on the slot as the default/easy path; intents are the *interceptable* path. Don't
  force indirection where unneeded.
- DEFER the 4-phase combat chain (AttackStarted→Connected→Calculated→Applied). Add each phase event
  only when its real subsystem (hit windows, armor/mitigation, crit) lands. None exist yet.
- Do the action-bar intent refactor BEFORE building combat abilities — cost asymmetry is decisive
  (flip 1 capability now vs. rewrite 20 abilities + rules later). Nameplate doesn't touch this path.
- Per-entity cancellation (entity silenced) can't use the single global LogicState string — lean on
  the existing per-entity `.behavior.ron` FSM rather than inventing a condition DSL.

**String namespace risk (separate, ship independently):** unmatched events silently vanish
(`match_rules` line ~69 just debug! and drops). ~16 namespaces today (entity/npc/inventory/stat/
dialogue/target/container/action_bar/audio/scene/item.*). Fix is TOOLING not runtime: extend
`ironhold_cli validate` to cross-check every `on:` event against emitted events + designer EmitEvent
strings; flag typos and orphans (extend existing `--strict` orphan detection). Plus document the
noun.verb:{id} convention as a contract in docs/30_runtime_events_and_logic.md.

See [[arch_decisions]], [[npc_state_machine_design]], [[capability_patterns]].

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

# Feature: Intent Event Layer

_Status: Draft_
_Planned at: `f66f7b2` (2026-06-23)_

## What

Add an **intent event** emission step to `action_bar.rs` so that ability activation flows through the interpreter before committing — making ability cancellation, modification, and conditional routing fully designer-reachable from RON.

This is a narrow, targeted fix. It does not change the interpreter, the Action type, or existing event strings. It closes the one capability that currently bypasses the Message → Interpreter → Action → Executor pipeline.

---

## Why

`action_bar.rs` is the only capability that validates and pushes actions directly to the `ActionQueue` without going through the interpreter. This means:

- A designer cannot cancel an attack when a player is silenced/frozen (must write Rust).
- A designer cannot redirect ability slot 1 to a different action based on game state.
- No `when:` / `LogicState` gate applies to ability execution.

Every other user-initiated event flows through the interpreter. The action bar is an inconsistency that will compound as combat features are built.

The research basis: RPG Event Taxonomy investigation (`planning/investigations/rpg_event_taxonomy.md`); system-architect assessment 2026-06-23.

---

## Approach

### Current flow (bypasses interpreter)

```
ButtonPressed("slot_1")
  → UiEvent::ButtonPressed
  → action_bar_system validates (cooldown, resource)
  → pushes do_actions directly to ActionQueue        ← bypasses interpreter
  → action_executor_system executes
  → fires "action_bar.activated:1" result event
```

### New flow (interpreter-routed)

```
ButtonPressed("slot_1")
  → UiEvent::ButtonPressed
  → action_bar_system validates (cooldown, resource)
  → fires GameEvent::Trigger("intent.slot.1:player_01")   ← NEW
  → message_interpreter_system / fsm_interpreter_system reads rules
  → dispatches do_actions from rule (or default slot do_actions)
  → action_executor_system executes
  → fires "action_bar.activated:1" result event
```

### Default rule (authoring the default path)

Without an explicit rule in `rules.ron`, the intent event produces no actions — which would break existing projects. The action bar must keep a fallback path:

Option A — action bar checks if a rule handled the intent (complex, requires two-phase dispatch).
Option B — action bar always fires `intent.slot.{n}:{entity}` AND still pushes `do_actions` as a default, but a designer rule can suppress the default via `CancelSlotDefault(n)` action.
Option C — keep `do_actions` on the slot as the default path; the intent event fires in parallel. A rule can override by registering `on: "intent.slot.1:player_01"` with `do_actions: [...]`. The slot's own `do_actions` is skipped if the interpreter handled the intent.

**Recommendation: Option C.** The interpreter checks for an `intent.slot.{n}:{entity}` handler; if one exists, it suppresses the slot's built-in `do_actions`. If none exists, the slot's `do_actions` runs as before. Zero migration cost for existing projects — no rule registered = current behaviour exactly.

### Intent event naming convention

```
intent.slot.{n}:{entity_id}        // ability slot activation
intent.interact:{entity_id}        // player presses interact key near target
intent.attack:{entity_id}          // direct attack (non-slot)
```

`{n}` is the slot number (1-indexed), `{entity_id}` is the acting entity's spawn ID.

### Designer usage

```ron
// Cancel attack when player has "silenced" status effect
( on: "intent.slot.1:player_01", when: "silenced", do_actions: [
    ShowFloatingText(entity: "player_01", text: "Silenced!"),
    // no attack action — intent is consumed with no effect
] )

// Redirect slot 1 to a different ability in "berserk" state
( on: "intent.slot.1:player_01", when: "berserk", do_actions: [
    PlayAnimation("rage_strike"),
    ModifyStat(key: "enemy.health", delta: -25.0),
    EmitEvent("combat.hit:player_01:{target}"),
] )

// Default (no rule) — slot's own do_actions runs unchanged
```

---

## Schema changes

None. Intent events use the existing `GameEvent::Trigger(String)` bus — no new types.

New `Action` variant (optional, for the slot-suppression mechanism):
```rust
// If Option C is used: no new action needed — interpreter "consuming" the intent suppresses the default.
// If a CancelDefault action is needed: minimal addition to action_executor_system.
```

---

## Rust changes

- `capabilities/action_bar.rs` — emit `GameEvent::Trigger("intent.slot.{n}:{entity_id}")` in the slot activation path, before or instead of pushing `do_actions` to `ActionQueue`
- `runtime/scene_manager/message_interpreter.rs` — add logic to check if an intent event was handled; suppress slot's `do_actions` if a matching rule was found
- No schema changes; no new Action variants required for the basic case

---

## Tasks

- [ ] Design decision: Option A, B, or C for default-path suppression (recommendation: C)
- [ ] Emit `intent.slot.{n}:{entity_id}` in `action_bar.rs` activation path
- [ ] Interpreter: detect intent handler and suppress slot default `do_actions` when a rule matches
- [ ] Add `intent.interact:{entity_id}` emission to interactable system (same pattern)
- [ ] Integration test: slot fires do_actions when no rule; slot is suppressed when a rule matches the intent
- [ ] Demo: add a silenced-state test rule in `entity_logic_demo` or `3rd_person_game_demo`
- [ ] Docs: add `intent.slot.{n}:{entity}` to `docs/30_runtime_events_and_logic.md` event catalogue

---

## Acceptance criteria

- Given no rule matches `intent.slot.1:{entity}`, the slot's own `do_actions` executes exactly as today.
- Given a rule matches `intent.slot.1:{entity}`, the rule's `do_actions` executes and the slot's built-in `do_actions` is suppressed.
- Given `when: "silenced"` on the matching rule and the entity is not in that state, the rule does not fire and the slot's `do_actions` executes.
- Given `when: "silenced"` and the entity IS in that state, the slot's `do_actions` is suppressed and the rule's `do_actions` runs.
- Existing projects with no intent rules are unaffected — behaviour identical to pre-feature.

---

## Open questions

- Should `intent.interact:{entity_id}` be added at the same time? Likely yes — same pattern, same effort, consistent coverage.
- Should intent events appear in the CLI `query events` output? Yes — they should be listed alongside result events in the event catalogue.
- Multi-phase combat chain (`AttackStarted → AttackConnected → DamageCalculated → DamageApplied`) is explicitly out of scope here. It belongs in the combat system design when damage formulas exist.

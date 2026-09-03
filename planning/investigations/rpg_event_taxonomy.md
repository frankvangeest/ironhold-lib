# Investigation: RPG Event Taxonomy — Alignment with ironhold

_Investigated at `4c7d2eb` (2026-06-23)_
_Source: `RPG_Event_Taxonomy.xlsx` (93 events, 16 categories)_

## Summary

The research thesis — "separate intent, execution, and result events" — is largely already implemented by ironhold's Message → Interpreter → Action → Executor pipeline. One genuine gap exists: `action_bar.rs` validates and commits ability execution entirely inside Rust, bypassing the interpreter. That single capability violates the project's own architecture rule and blocks designer-authored ability cancellation.

**Fix now, narrowly.** The action bar emits an `intent.{slot}` GameEvent before committing, and a rule maps intent → actions. Everything else in the taxonomy either already works or should be deferred until the subsystems that need it exist.

---

## How ironhold maps to the three phases

### Intent phase
In the taxonomy: `MoveIntent`, `AttackIntent`, `CastSpellIntent`, `InteractIntent`, `UseItemIntent`

**Current state — partially there, one gap:**

All input that flows through the interpreter already creates a natural intent layer. A `UiEvent::ButtonPressed("attack")` fires, the interpreter reads it, and a designer-authored rule decides what `Action`s result. That *is* intent → execution.

The gap: `action_bar.rs` is a special case. It validates cooldown and resource costs internally, then pushes directly to the `ActionQueue` — bypassing the interpreter entirely. No RON rule can intercept between "button pressed" and "ability executes." A silenced player cannot have their attack cancelled by a designer-authored rule; that logic must be written in Rust.

**The fix** (see `planning/features/intent_event_layer.md`): emit `intent.{slot}:{entity}` and let the interpreter dispatch. The action bar's own `do_actions` stays as the default. Cancellation is then expressible with existing `when:` / `LogicState` gates — no new Rust required.

### Execution phase
In the taxonomy: `AttackStarted`, `MovementStarted`, `MovementChanged`

**Current state — covered.** `action_executor_system` is the single execution point for all Actions. Every action that reaches it executes; there's no second validation layer to bypass. This is correct.

### Result phase
In the taxonomy: `AttackConnected`, `DamageCalculated`, `DamageApplied`, `EntityKilled`, `ItemAdded`, `QuestCompleted`…

**Current state — covered, but ad-hoc.** Capabilities emit `GameEvent::Trigger(String)` after they execute. Real examples already in use:
```
entity.attacked:{id}       entity.interacted:{id}
entity.collected:{id}      inventory.added:{entity}:{item_key}:{count}
stat.{key}.depleted        dialogue.started:{npc_id}
container.looted:{id}      target.changed:{id}
npc.player_spotted:{id}    npc.player_lost:{id}
```
These strings are free-form with no enforced convention, no categories, and no wildcard matching. Silent typos in RON rules produce dead rules with no error.

---

## Taxonomy coverage by category

| Taxonomy Category | Alignment | Notes |
|---|---|---|
| **Core Engine** | ✓ Covered | `SceneEvent` (Requested/Loaded/Ready/Unloading) + `GamePaused`/`GameResumed` via GameEvent |
| **Scene** | ✓ Covered | `scene.requested`, `scene.loaded`, `scene.ready`, `scene.unloading` |
| **Input** | ✓ Covered | `InputActionMessage` (Move/Jump/Run) + `UiEvent::ButtonPressed` |
| **Intent** | ⚠ Partial | All events flow through interpreter *except* action bar (the gap) |
| **Movement** | ✓ Partial | `player.jumped` exists; `MovementStarted`/`Stopped` not emitted (no subscriber needs them yet) |
| **Combat** | ⚠ Partial | `entity.attacked:{id}` exists; no `AttackStarted`, `DamageCalculated`, `AttackConnected` phases |
| **Health** | ✓ Covered | `stat.{key}.depleted` + threshold events; `HealthChanged` implicit in stat update |
| **Status** | — Not built | Status effect system not yet implemented |
| **Inventory** | ✓ Covered | `inventory.added`, `inventory.removed`, `inventory.full`, `inventory.transferred` |
| **Quest** | — Not built | Quest system not yet implemented |
| **Dialogue** | ✓ Covered | `dialogue.started`, `dialogue.ended`; choice events not yet emitted |
| **Story** | ✓ Covered | `GameVariable` / `SetVariable` + `LogicState` FSM — equivalent to `StoryFlagSet` |
| **World** | — Not built | No day-night, weather, or region system |
| **NPC** | ✓ Covered | `npc.player_spotted`, `npc.player_lost`, `npc.investigating`, `npc.player_reached` |
| **AI** | ✓ Covered | `target.changed`, `target.cleared`; behavior state via NPC AI FSM |
| **Abilities** | ⚠ Partial | Action bar `do_actions` covers activation; no `AbilityUnlocked`, `AbilityCooldownStarted` events |
| **Progression** | — Not built | No XP/leveling system |
| **Save/Load** | — Not built | No save/load system (Icebox) |
| **UI** | ✓ Covered | `UiEvent::ButtonPressed`; `ui.opened`/`ui.closed` not emitted but easily added |
| **Audio** | ✓ Covered | `audio.muted`, `audio.unmuted`, `audio.volume_changed` |
| **Animation** | ⚠ Partial | `AnimationCompleted` not emitted; `AnimationFrameEvent` (hit-window) not possible |
| **Systemic** | ✓ Covered | `EmitEventAfterDelay` = `TimerElapsed`; `EmitEvent` = `ConditionMet` |

---

## The string-key namespace problem

Free-string event names create two failure modes:
1. **Silent dead rules** — `"entity.interacted:merchant_1"` (underscore) never matches `"entity.interacted:merchant_01"` (zero-padded). No error, no warning.
2. **Drift over time** — as capabilities multiply, conventions drift. `"entity.attacked"` vs `"attack.landed"` vs `"combat.hit"` for the same concept.

The fix is a **CLI `--strict` validator** that reads all RON `on:` fields and checks them against a known event registry. This is pure upside — no runtime change needed. Already listed in Queued › Designer Experience backlog.

---

## The multi-phase combat chain

The taxonomy's `AttackStarted → AttackConnected → DamageCalculated → DamageApplied` chain enables:
- Hooking animation frame events into hit windows
- Intercepting damage calculation for armour/crit modifiers in RON
- Reacting to misses/dodges/parries separately

**This does not exist and should not be built yet.** It requires a combat subsystem with hit detection, damage formula evaluation, and defence mechanics — none of which are implemented. Building the event chain now without the subsystems that need it produces phantom infrastructure. Add it when the combat system is designed.

---

## Recommended actions

1. **Fix the action bar gap** — emit `intent.{slot}:{entity}` before committing. Small Rust change, big designer impact. Do this before authoring any more abilities. See `planning/features/intent_event_layer.md`.

2. **CLI event-namespace validator** — validate `on:` strings in RON against a registry of known emitted events. Already Queued (Designer Experience). No runtime change needed.

3. **Defer multi-phase combat chain** — `AttackStarted → Connected → Calculated → Applied` belongs in the combat system design, not here.

4. **Document the event catalogue** — add a reference table of all emitted `GameEvent::Trigger` strings to `docs/30_runtime_events_and_logic.md`. Designers can't reliably author rules without knowing what events exist.

5. **Adopt the taxonomy naming conventions** — new capabilities should follow the category pattern from the taxonomy as a style guide (not a runtime type). `intent.attack:{entity}`, `combat.hit:{attacker}:{target}`, etc. rather than ad hoc strings.

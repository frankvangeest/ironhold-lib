---
name: intent-event-layer
description: action_bar intent/flush layer — per-frame system but allocation-free on idle; HashMap/HashSet stay empty when no slot pressed
metadata:
  type: project
---

Intent event layer (`capabilities/action_bar.rs` + `runtime/scene_manager/message_interpreter.rs`).

Two resources: `PendingIntentActions(HashMap<String,Vec<Action>>)`, `HandledIntentSlots(HashSet<String>)`. Both stay empty on frames with no digit-key press.

`flush_pending_intent_system` runs every frame in the chained Update set (after all 3 interpreters, before action_executor). Idle cost: one `drain()` over an empty HashMap (O(0), no alloc, no DerefMut churn beyond the unconditional ResMut access) + `handled.0.clear()` on empty set (no-op). Negligible.

Interpreter systems each gained `ResMut<HandledIntentSlots>` and call `handled.0.insert(slot_key)` only when an `intent.slot.*` event matches. `intent_slot_key()` does a `strip_prefix`/`split` + one `to_string()` — allocates a short String only on actual match (per slot activation, not per frame).

**Why allocation-free on idle:** action flow only triggers on `keys.just_pressed` digit/letter, which early-returns. HashMap.insert + the `do_actions.clone()`/`rewrite_target` Vec build happen only on a validated slot press (player input cadence, ~0-1/frame). HashSet capacity stays at 0 until first insert.

**How to apply:** This is the canonical "per-frame system, zero idle allocation" pattern — do not flag it. If reviewing changes here, the risk to watch is anything that makes `PendingIntentActions`/`HandledIntentSlots` retain capacity or entries across frames (they must be drained/cleared every frame), or moving the `rewrite_target` clone out of the input-gated branch into per-frame code.

Related: [[project_rewrite_target]] (rewrite_target canonical copy lives in message_interpreter.rs), ActionQueue FIFO ordering preserved (flush pushes in stored Vec order).

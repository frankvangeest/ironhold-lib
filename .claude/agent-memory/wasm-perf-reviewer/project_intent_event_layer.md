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

**Custom-hotkeys change (feature/action-bar-custom-hotkeys, reviewed 2026-07-15):** `action_bar_input_system` is gated by `run_if(any_action_slots)` (no-op when no `ActionSlotUi` entities exist). Old key-match did a fixed ~11-entry `DIGIT_KEYS` array scan of `just_pressed` + a second linear string-compare scan over slots. Now a single `slots.iter().find(|s| s.resolved_key.is_some_and(|kc| keys.just_pressed(kc)))` — one linear scan over slots checking a pre-resolved `KeyCode` (`ActionSlotUi.resolved_key: Option<KeyCode>`, a `Copy` enum, no alloc). Neutral-to-positive for frame time (fewer comparisons in the common case; `find` short-circuits; no new alloc, query, or system). Key resolution (`InputMap::parse_key`) + duplicate-key `warn!` (via a per-bar `HashMap<KeyCode,String>`) happen once in the scene loader's `spawn_ui_element_node` — per-scene-load, not per-frame. `InputMap::parse_key`/`KeyCode` are WASM-safe (already used across the codebase). Fire-first single-scan semantics deliberately kept for the single-bar case; per-player (Phase 2) will restructure into a loop over all pressed slots — watch that that loop stays input-gated and doesn't collect per-frame.

Related: [[project_rewrite_target]] (rewrite_target canonical copy lives in message_interpreter.rs), ActionQueue FIFO ordering preserved (flush pushes in stored Vec order).

---
name: delayed-event-staleness-class
description: DelayedEventQueue has no cancellation, is never cleared on scene load, and its events are dropped if the FSM left the state that handles them — plus initial_items is spawn-only with no refill action
metadata:
  type: project
---

Two engine facts that jointly break "timer-driven lifecycle" RON designs (verified 2026-08-24 in
`ironhold-lib-monster-corpse-loot-v1`):

1. **`DelayedEventQueue` is `pub struct DelayedEventQueue(pub Vec<(f32, String)>)`
   (`runtime/scene_manager/mod.rs`) — no cancellation action, no owning entity, no generation
   tag.** An FSM cannot un-arm a timer it armed on state entry. Splitting a state in two
   (`dead_full`/`dead_looted`) only fixes *half* the stale-timer problem: state-scoped `on:`/
   `transitions:` matching means a stale event can no longer fire the wrong *action*, but it is
   still a valid *trigger* in any state the entity can re-enter. So the real invariant to enforce
   in RON is: **at most one timer per event name per lifecycle iteration, with delay ≤ the shortest
   possible cycle length.** Two states arming the same event name with different delays (20s
   ambient vs 5s post-loot) violates it — a re-kill inside the (20 − 5)s window gets a premature
   transition from the first death's timer.
   `docs/30_runtime_events_and_logic.md`'s "loot corpse" pattern section asserts this is harmless
   and reasons only about the `alive` case; it is wrong for the re-enter-`dead_full` case.

2. **`InventoryContainerDef.initial_items` is applied only at spawn**
   (`runtime/scene_manager/entity_spawner.rs`, ~line 74) and there is **no action that writes
   container contents** (no `AddItemToContainer`/`SetInventory`/`RefillContainer` in
   `schema/actions.rs`). `ResetToSpawn` does not touch `Inventory`. So any hide-and-revive
   (non-despawn) container is lootable exactly once per scene load; a despawn+respawn design gets
   refill for free because each instance is a fresh entity.

3. **A delayed event is FSM-state-blind, scene-blind and pause-blind.** `tick_delayed_events_system`
   (`lib.rs`) ticks off plain `Res<Time>`; nothing pauses it (no `Time<Virtual>::pause()` anywhere in
   core) and nothing clears `DelayedEventQueue` on `LoadScene`. Meanwhile a global rule under
   `states: [(name: "playing", on: [...])]` only matches while the FSM *is* in `playing`. So a long
   timer (e.g. a 60s respawn) armed in `playing` and handled only by `playing.on:` is **silently
   dropped** if the FSM sits in `paused`/`menu` when it fires — the respawn never happens for the
   rest of the session, no warning. Verified 2026-08-26 on monster_corpse_loot v2, where `paused`
   is just a `LoadSceneOverlay` state that doesn't stop world time.
   **Rule: any handler for a delayed event whose delay can outlive the current FSM state belongs in
   `global_on:`, not a per-state `on:`.** (`global_on` is a real, used field — see
   `3rd_person_game_demo/logic/state_machine.ron`'s audio rules.)

**Why:** all three invalidate otherwise-reasonable RON-only lifecycle designs and are invisible from
the schema docs.

**How to apply:** cite (1) whenever a design arms the same delayed event from more than one state,
or arms a timer longer than the cycle it lives in; cite (2) whenever a reusable container's
contents need to come back. See [[spawn_id_lifecycle_invariants]] for the adjacent
spawn/despawn/`container.looted` facts.

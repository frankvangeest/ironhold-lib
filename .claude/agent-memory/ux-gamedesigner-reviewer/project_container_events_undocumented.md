---
name: container-events-undocumented
description: container.* events are now documented in docs/30 (lines ~107-109); remaining gaps are the missing "Lootable corpse" walkthrough section, trigger_zone needing explicit entity.exited wiring, and initial_items never refilling
metadata:
  type: project
---

**Status update (2026-08-24, `feature/monster-corpse-loot-v1`):** the three `container.*` events
(`container.opened:{id}`, `container.closed`, `container.looted:{id}`) ARE now documented in
`docs/30_runtime_events_and_logic.md`'s event list (~lines 107-109), including the "does not fire on
an already-empty container" caveat. The first shipped `OpenContainer("{self}")` example also now
exists: `3rd_person_game_demo/behaviors/enemy_zombie.behavior.ron` (`dead_full` state).

Remaining gaps to check on any loot/container/corpse work:
- `docs/30`'s hide-vs-`Despawn` guidance table (~line 504) links to a section titled
  **"Lootable corpse (loot-on-death)" that does not exist** in any docs file — dangling reference.
- **`trigger_zone:` alone does NOT auto-close a container panel.** The close-on-walk-away behavior
  requires an explicit `entity.exited:{id}` → `CloseContainer` handler (only `chest_01`/
  `chest_02_spawned` in `logic/state_machine.ron` have one). A corpse/container that has
  `trigger_zone` but no `entity.exited` handler silently never closes.
- **`initial_items` are placed at spawn time only.** The hide+`ResetToSpawn` respawn pattern never
  re-spawns the entity, so a looted container/corpse stays empty for the rest of the session — and
  because `container.looted` can't fire on an empty container, a second kill can never reach the
  `looted` state. No action exists to refill an inventory.

Canonical loot-container prefab shape (`3rd_person_game_demo/prefabs/prefabs.ron` `chest_01`):
`kind: Prop` + `nameplate: false` + `interactable(radius, hint_text: "Loot")` +
`trigger_zone(radius)` + `colliders` + `click_selectable` + `indicator_category` +
`inventory(max_slots, initial_items)`.

**How to apply:** on loot/corpse features, verify the `entity.exited` close handler exists, ask
whether repeat-loot is expected (it isn't supported today), and require the promised docs section to
actually be written, not just linked. Related: [[inventory-item-system]].

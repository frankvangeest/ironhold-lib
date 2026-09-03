---
name: container-events-undocumented
description: container.* events are documented in docs/30 and the dangling "Lootable corpse" link now resolves; remaining gaps are trigger_zone needing explicit entity.exited wiring and initial_items never refilling
metadata:
  type: project
---

The three `container.*` events (`container.opened:{id}`, `container.closed`,
`container.looted:{id}`) are documented in `docs/30_runtime_events_and_logic.md`'s event list,
including the "does not fire on an already-empty container" caveat. The hide-vs-`Despawn` guidance
table's link to "Lootable corpse (loot-on-death)" now resolves to a real, fully-written section in
the same doc (no longer dangling) — see [[corpse-loot-v2-pattern]].

Remaining gaps to check on any loot/container/corpse work:
- **`trigger_zone:` alone does NOT auto-close a container panel.** The close-on-walk-away behavior
  requires an explicit `entity.exited:{id}` → `CloseContainer` handler. Still only
  `chest_01`/`chest_02_spawned` in `3rd_person_game_demo/logic/state_machine.ron` have one
  (verified — no other container/corpse entity in the project has a matching `entity.exited`
  handler). A corpse/container that has `trigger_zone` but no `entity.exited` handler silently
  never closes.
- **`initial_items` are placed at spawn time only.** No `RefillContainer`-style action exists
  (`entity_spawner.rs` only consumes `initial_items` once, at spawn). The hide+`ResetToSpawn`
  respawn pattern never re-spawns the entity, so a looted container/corpse stays empty for the rest
  of the session — and because `container.looted` can't fire on an empty container, a second kill
  can never reach the `looted` state. No action exists to refill an inventory.

Canonical loot-container prefab shape (`3rd_person_game_demo/prefabs/prefabs.ron` `chest_01`):
`kind: Prop` + `nameplate: false` + `interactable(radius, hint_text: "Loot")` +
`trigger_zone(radius)` + `colliders` + `click_selectable` + `indicator_category` +
`inventory(max_slots, initial_items)`.

**How to apply:** on loot/corpse features, verify the `entity.exited` close handler exists, ask
whether repeat-loot is expected (it isn't supported today), and require any new "docs section
coming" promise to actually be written before merge, not just linked. Related:
[[inventory-item-system]], [[corpse-loot-v2-pattern]].

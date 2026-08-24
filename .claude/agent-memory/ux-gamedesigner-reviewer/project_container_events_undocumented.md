---
name: container-events-undocumented
description: container.opened/closed/looted events are emitted by the executor but appear in NO docs/ file; container wiring only has literal-id examples, none for dynamically spawned containers
metadata:
  type: project
---

`Action::OpenContainer`/`TakeAllFromContainer` emit `container.opened:{id}`, `container.closed`,
`container.looted:{id}` (action_executor.rs ~1438/1447/1506). **None of these three appear anywhere
in `docs/`** — not in `docs/30_runtime_events_and_logic.md`'s event tables, not in
`docs/20_data_formats.md`. Any feature that keys off "the player looted this" (corpse decay, quest
triggers, chest-emptied VFX) is building on an event a designer cannot discover.

`OpenContainer` IS in `rewrite_self` (message_interpreter.rs ~280), so `OpenContainer("{self}")`
works in a behavior file — but **every shipped example wires containers by literal id in
`logic/state_machine.ron`** (`3rd_person_game_demo` chest_01 / chest_02_spawned). There is no
shipped example of a container wired from a `*.behavior.ron` via `{self}`, which is the only
workable pattern for containers spawned at runtime with a generated/derived id.

Canonical loot-container prefab shape (`3rd_person_game_demo/prefabs/prefabs.ron` `chest_01`):
`kind: Prop` + `nameplate: false` + `interactable(radius, hint_text: "Loot")` +
`trigger_zone(radius)` (this is what makes walk-away `CloseContainer` possible) + `colliders` +
`click_selectable` + `indicator_category` + `inventory(max_slots, initial_items)`. A new
container prefab that omits `trigger_zone` silently loses the close-on-leave behavior.

**Why:** loot/container features keep getting proposed on top of this undocumented event surface
and the literal-id wiring example; both gaps hit the designer the moment the container id isn't
known at authoring time.

**How to apply:** on any loot/container/corpse feature, require (a) the three `container.*` events
be added to `docs/30_runtime_events_and_logic.md`'s event table, and (b) a shipped
`OpenContainer("{self}")` behavior-file example. Related: [[inventory-item-system]].

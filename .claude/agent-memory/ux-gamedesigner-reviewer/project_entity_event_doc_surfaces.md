---
name: entity-event-doc-surfaces
description: The 5 doc surfaces any new entity.* event / InteractableDef field must touch, plus the long-standing under-documented interactable row
metadata:
  type: project
---

Adding a new `entity.*` event or a new `InteractableDef` field means editing **five** places, not
the two most plans list:

1. `docs/20_data_formats.md` ~1940 — the `PrefabDef` `interactable` row. **This row still says
   "Field: `radius: f32`." and has never listed `hint_text`**, despite `hint_text` shipping in
   three doc examples (~3984, ~4650, ~4658) and in `3rd_person_game_demo` prefabs. Any change here
   should fix the row wholesale into a proper sub-field list, not append one more undocumented
   sibling.
2. `docs/30_runtime_events_and_logic.md` ~104-106 — the `entity.*` bullet list.
3. `docs/30_runtime_events_and_logic.md` ~470-471 — the component → emitted-event table
   (`Interactable | interactable: (radius: 2.5) | entity.interacted:{id}`). Routinely missed.
4. `docs/STATUS.md` ~57 (Interactable feature row, spells out `Interactable { radius }`) and ~113
   (entity event list). Routinely missed.
5. The shipped example project — `3rd_person_game_demo` is the canonical home for
   interactable+inventory features (see [[inventory-item-system]]).

Precedent for what happens when surfaces are skipped: `shop.insufficient_funds` is emitted by
`action_executor.rs` but appears in **no** docs file. "You can't do X" events are the class most
likely to ship undocumented.

**Why:** designers discover interactable fields only from these tables; an undocumented sibling
field compounds (hint_text has been invisible for multiple release cycles).
**How to apply:** when reviewing any interactable/entity-event plan, check all five and flag each
missing one explicitly by line number.

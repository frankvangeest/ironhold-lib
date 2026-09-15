---
name: entity-event-doc-surfaces
description: The 6 doc surfaces any new entity.* event / InteractableDef field must touch; interactable row now fixed, STATUS.md event list + docs/60 check list are the two that still lag
metadata:
  type: project
---

Adding a new `entity.*` event or a new `InteractableDef` field means editing **six** places, not
the two most plans list:

1. `docs/20_data_formats.md` ~1940 — the `PrefabDef` `interactable` row. **FIXED as of the
   `item_gated_interactable` feature (2026-09-14):** the row is now a proper sub-field list
   covering `radius` / `hint_text` (documented as "not yet rendered, reserved for a future UI
   pass") / `requires_item`. Do not re-flag `hint_text` as undocumented.
2. `docs/30_runtime_events_and_logic.md` ~104-107 — the `entity.*` bullet list.
3. `docs/30_runtime_events_and_logic.md` ~470-473 — the component → emitted-event table.
4. `docs/STATUS.md` — **TWO separate spots**, and the second is the one that keeps getting missed:
   ~57 (Interactable feature row) *and* ~110-114 ("Entity messages (Beta 0.4)" bullet list).
   `item_gated_interactable` updated ~57 but not the ~113 list. Check both, every time.
5. `docs/60_contributing.md` ~245-246 — the `ironhold_cli validate` "Checks performed" list.
   Any new cross-file reference check must be appended here or designers never learn the safety
   net exists. `item_gated_interactable`'s `requires_item` check shipped without this.
6. The shipped example project — `3rd_person_game_demo` is the canonical home for
   interactable+inventory features (see [[inventory-item-system]]). Also cross-link *from* the
   docs to it: docs house style cites its canonical example inline (e.g. the `target.changed` row
   cites `3rd_person_game_demo/logic/rules.ron`), but `requires_item` shipped with no such
   pointer to `seal_door`.

Long-standing precedent for skipped surfaces: `shop.insufficient_funds` and
`player.attack_missed` are both emitted by the engine and appear in **no** docs event list
(`player.attack_missed` is emitted by `capabilities/interactable.rs` when interact hits nothing).
"You can't do X" events are the class most likely to ship undocumented.

**Why:** designers discover interactable fields only from these tables; an undocumented sibling
field compounds.
**How to apply:** when reviewing any interactable/entity-event plan, check all six and flag each
missing one explicitly by line number.

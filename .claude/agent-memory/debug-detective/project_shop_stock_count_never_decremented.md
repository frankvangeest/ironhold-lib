---
name: shop-stock-count-never-decremented
description: BuyItem only checks stock_count == Some(0) and never decrements, so any finite stock (e.g. old_key stock_count 1) sells unlimited; requires_item never consumes
metadata:
  type: project
---

`Action::BuyItem` (`action_executor.rs`, ~1556) refuses only on `stock_count == Some(0)` and never
decrements it; `merchant.stock` is read from the immutable `PrefabCatalog` clone each time. So
`stock_count: 1` behaves as unlimited, and the shop label keeps showing `[1]`. Schema doc
(`catalog.rs` ~1692, "Finite stock — restocks at scene load") is false. Found 2026-11-14 while
tracing a "multiple keys, gate won't open" playtest report in 3rd_person_game_demo.

**Why:** a fix needs per-merchant runtime state (a component or resource keyed by merchant spawn
id), not a catalog mutation.

**How to apply:** if a report involves "more of an item than the shop should sell", start here.
Related: `interactable.requires_item` is check-only and never consumes (documented); a designer
must add `RemoveItem(entity: "player", item_key: ..)` to the `entity.interacted:{id}` handler.
`interactable_system` early-returns on `panels_open > 0`, so pressing F with the inventory open
does nothing at all (no blocked event either) — see [[panels-open-refcount-leak]].

---
name: project_item_txn_s2_plan_review
description: Item system S2 (transaction primitive) plan-review 2026-11-14 - built-in hardcoded shop texts, docs/30 missing BuyItem/item.bought, old_key quest strand, generic+specific event pairs
metadata:
  type: project
---

Plan-reviewed `planning/features/item_transaction_primitive.md` at 9fa2ef3 (2026-11-14). Facts worth keeping:

- Engine ALREADY shows hardcoded English floating text on BuyItem: "Not enough gold!" (insufficient funds) and
  "Bought {display_name}!" (success), at offset (0,2.2,0) over the player. Undocumented anywhere in docs/, not
  overridable, and says "gold" whatever the currency_stat. Sold-out today is silent (info log only).
- Pre-existing doc gaps: docs/30 action list has no BuyItem / TakeAllFromContainer / OpenContainer / CloseContainer
  entries; docs/30 event list has no `item.bought` or `shop.insufficient_funds`. docs/20 inventory events table (~5053)
  lists only 4 inventory.* events. STATUS.md has no inventory/shop row.
- Quest strand in 3rd_person_game_demo if stock persists for the session: buy old_key (stock_count 1) -> open
  seal_door (RemoveItem consumes key) -> Start Menu -> select_male reloads main: door is back, key gone, sold out.
  Chests reset on LoadScene while stock would not - asymmetry to document.
- Event matching is exact-only, so `shop.sold_out:{item}` alone forces one rule per item; recommended emitting a
  generic + specific pair (precedents: target.changed / target.changed:<id>, lobby generic-then-:<reason>).
- RemoveItem is "up to N": a quest turn-in of exactly 3 pelts is not authorable (no count condition in RON).
- TakeAll credits coins and still emits `inventory.added:player:gold_coin:N`; a coin whose stat is missing stays put.
- `stats.ron` gold comment in 3rd_person_game_demo says "future buy/sell (v1.1)" - stale.

**How to apply:** on S3-S6 reviews, check these were closed; see [[inventory-item-system]], [[project_panel_world_blocking]].

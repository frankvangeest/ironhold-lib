---
name: project_item_txn_s2_plan_review
description: Item system S2a (transaction primitive) plan+impl review 2026-11-14 - built-in buy messages use raw currency_stat KEY, shop.no_room has no generic form, STATUS row calls transfer all-or-nothing (wrong), Fill Bag is one-way
metadata:
  type: project
---

Plan-reviewed at 9fa2ef3, impl-reviewed on feature/item_transaction_primitive (2026-11-14). Facts worth keeping:

- Built-in floating texts on BuyItem ("Bought {display_name}!", "Not enough {currency_stat KEY}!", "Sold out!",
  "Bag is full!") over the primary player; not overridable until S3's default-backed events. The plan said
  "currency display name" but the code formats the raw stat key - a snake_case key leaks to players.
- Event families are inconsistent: sold_out/insufficient_funds/remove_failed have generic+specific; `shop.no_room`
  and `inventory.transfer_failed` only specific. Exact-match means a bare `shop.no_room` rule silently never fires.
- TransferItem is partial (moves what fits), NOT all-or-nothing; on zero moved it emits inventory.full:{to} AND
  transfer_failed. Coin TransferItem to player also emits inventory.added:player:{coin}:{n}.
- "What survives LoadScene" table lives only in docs/20 ~5071 (inventory events block); linked from ShopPanel note
  ~1399 by "below" only; LoadScene action entry (docs/30 ~620) and initial_items row don't point to it.
- Bag survives LoadScene and there is no drop/destroy until S4, so a debug "fill bag" button is one-way for the session.
- No shipped project and no doc RON example uses any shop.* / remove_failed event or all_or_nothing.
- docs/30 ~140 intent.slot line has a duplicated "and passes all checks" (pre-existing).

**How to apply:** on S2b-S6 reviews, check these were closed; see [[inventory-item-system]], [[project_panel_world_blocking]], [[project_ron_comments_cite_dev_paths]].

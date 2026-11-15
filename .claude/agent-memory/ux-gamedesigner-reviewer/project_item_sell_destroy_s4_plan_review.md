---
name: project_item_sell_destroy_s4_plan_review
description: Item system S4 (SellItem/DestroyItem, bag slot menu) plan review 2026-11-15 - ShopEntry sell_price:0 legacy meaning, overrides can't express "all", non-stackable "all" footgun, Escape=toggle_pause in demo
metadata:
  type: project
---

Plan-reviewed 2026-11-15 at a65e696 (planning/features/item_sell_destroy.md). Verdict: Needs more design work (small).

Points to check at impl review:
- `ShopEntry.sell_price: 0` was authored (demo old_key, docs/20 key_iron example ~5050) meaning "not bought".
  Reading Some(0) as "takes it for free" makes a quest key sellable -> soft-lock. I recommended Some(0) = "not bought here".
- Default `destroy_item:`/`sell_stack:` act on "all N", but the event has no count and DestroyItem/SellItem count
  defaults to 1: an override rule cannot reproduce the built-in. Asked for an `all: bool` field (bool house style).
- "All" across every stack/slot is wrong for non-stackable items (3 swords: clicking one offers "Destroy all 3").
  Recommended clicked-slot quantity.
- Demo binds Escape to toggle_pause (KeyP too), so "Escape closes the menu" also pauses.
- Empty menu (old_key: no use, no price, not destroyable) left-click = silent; asked for an item-name header row.
- Stale "click or right-click to use" text: items.ron line 13, docs/20 ~5009 and ~5118, state_machine.ron ~178.
- Sell message reuses raw currency_stat key (S2a gap); sell_refused has no default message.

**How to apply:** at S4 impl review and S5/S6 (bar slots), recheck these; see [[project_item_use_s3_plan_review]],
[[project_item_txn_s2_plan_review]], [[project_ui_label_button_font_and_clip]].

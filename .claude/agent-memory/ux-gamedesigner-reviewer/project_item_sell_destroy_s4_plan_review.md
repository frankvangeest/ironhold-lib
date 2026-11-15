---
name: project_item_sell_destroy_s4_plan_review
description: Item system S4 (SellItem/DestroyItem, bag slot menu) plan + impl review 2026-11-15 - plan findings honoured; open: stale "selling not implemented"/"click to use" lines in docs/20, red No button, "Can't carry any more" = gold max, unwrapped prompt width
metadata:
  type: project
---

Plan-reviewed 2026-11-15 at a65e696; impl-reviewed 2026-11-15 (worktree ironhold-lib-item_sell_destroy, diff vs 200021c).

Honoured at impl: ShopEntry sell_price Some(0) = "not bought here" (sell_price_for), `all: bool`, clicked-stack
quantity labels ("Sell 10 (50 gold)", header "Health Potion x10"), dimmed "Cannot be sold or destroyed" row, 40px
button rows, menu flips toward panel middle, Sell N + Destroy confirm, items.ron/state_machine.ron/HUD text fixed.

Open after impl review (recheck at merge / S5):
- docs/20 still says "Selling is still not implemented" (ShopPanel callout ~1400, Buying section ~5108) and
  "click to use" (~1361 InventoryPanel, ItemDef `use` row ~4946, example comment ~5011).
- Confirm rows colour No red (ROW_BG_DANGER on Destroy|No) - the safe choice looks dangerous; Yes is neutral.
- SellItem refusal "Can't carry any more" fires on currency stat max, reads as bag-full.
- Sold message omits the count ("Sold Health Potion for 50 gold" after selling 10).
- Prompt text has no max_width/wrap and repeats the item name: long names overflow a phone screen.
- Demo merchant ShopEntry sell_price == item sell_price, so the override is invisible in play; no SellItem bar slot,
  so refusal messages are unreachable in the demo; gold max 99999 makes the stat-max refusal untestable.
- Demo Escape = toggle_pause still the only keyboard way to close the menu (documented).

**How to apply:** at S5/S6 (bar item slots) recheck these; see [[project_item_use_s3_plan_review]],
[[project_item_txn_s2_plan_review]], [[project_ui_label_button_font_and_clip]].

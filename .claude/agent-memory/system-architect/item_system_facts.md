---
name: item-system-facts
description: Item system (inventory/container/shop/bar/currency/key items) facts and the "one-sided move" bug class; target model in planning/features/item_system_review.md (2026-11-14)
metadata:
  type: project
---

Verified at 5fd373c (2026-11-14), design doc `planning/features/item_system_review.md` (Draft, open questions for Frank).

**Bug class: "apply one side of an item move, ignore the other side's result".** Every item action hand-rolls
add_to_slots/remove_from_slots in `action_executor.rs`: TakeAll (fixed on unmerged `feature/take_all_overflow`),
BuyItem (price deducted before add, add result ignored, `item.bought` still fires), **TransferItem** (removes, ignores
add result, emits `inventory.transferred` anyway; found by this review, not yet in backlog). Proposed fix: one pure
clone-and-swap transaction primitive (`TxnOp` Take/Give/Stat/Stock), partial-by-design actions (AddItem, TakeAll)
as loops of single-stack transactions.

Non-obvious facts:
- Merchant stock has NO runtime copy; OpenShop/BuyItem re-read `MerchantDef` from the prefab catalog, so `stock_count`
  can never decrement. Schema comment "restocks at scene load" is false; docs row says "not yet implemented".
- `PlayerInventory.max_slots` is a lazy hardcoded 20, unrelated to the InventoryPanel's columns*rows.
- Currency conversion (`ItemDef.currency_stat`) happens only in TakeAll; BuyItem reads LoadedStats only while the action
  bar's `resolve_cost_source` prefers the player's StatMap: two currency rules.
- `panels_open` is a counter: OpenInventory always +1, every Close* always -1; demo `entity.exited:merchant_01 -> CloseShop`
  decrements on every walk-past. Same lesson as SimClock: use an idempotent set. Even fixed, an open inventory blocks
  interact by design (open question: inventory modality).
- S1 review (2026-11-14, `feature/item_panels_open_set`): set lives INSIDE `LoadedInventoryUi` (not a separate
  `OpenPanels` resource as the review proposed) — fine, scene_loader's whole-struct `Default` reset on Replace also clears it.
  Residual facts for S2-S6: (a) `ui.panel_opened/closed` stay non-edge (every Close* emits, e.g. walk-past CloseShop) and
  carry no panel kind; `set_panel_open` returning "changed" is the cheap hook for edge events. (b) `initially_hidden: false`
  Inventory/Shop panels spawn Visible but NOT in the set (no shipped RON uses it). (c) No Despawn teardown for an open
  shop's merchant (container has one). (d) `is_modal` uses `!matches!(Inventory)` so a new PanelKind silently defaults to modal.
  (e) collectible_system early-return leaves CollisionEvents unread, so a Started pair from while blocked can fire right
  after the panel closes (pre-existing).
- Shop Buy buttons emit `ui.button_pressed:buy_item:{key}`; FSM is exact-match, so one RON rule per stocked item.
  Proposed: "default-backed events" (generalise HandledIntentSlots) so a rule wins, else the engine runs the default.
- bevy_ui 0.18 `ui_focus_system` sets Pressed only for Left mouse / touch (focus.rs:176,188): right-click-to-use needs
  its own system; touch needs a menu. Inventory slots have no `Button`.

**How to apply:** any new item-moving action must go through the primitive (once built) and emit a failure event
instead of a success event on failure. See [[inventory-panel-triplication]], [[panel-input-blocking]],
[[event-pipeline-intent-layer]], [[stat-ownership-global-vs-instance]].

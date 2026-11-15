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

- S2 plan-review (2026-11-14, at 9fa2ef3) facts: `PlayerInventory.player_spawn_id` is set ONLY by the dynamic
  `Action::Spawn` arm (`action_executor.rs:299`), so for scene-placed players it is None ("Not enough gold!" text never
  shows) and it is no basis for "the player's own StatMap" — needs an explicit primary-player rule. Gold footer
  (`inventory_ui_system`) reads LoadedStats only: must use the same currency resolver. `apply_delta` clamps to
  `[min,max]`, so coin credit past max silently loses coins. `add_to_slots` treats an unknown item (or no catalog) as
  max_stack 99 — a `TxnError::UnknownItem` would break catalog-less projects. No existing tests cover BuyItem/
  TransferItem/AddItem at all (only TakeAll in corpse_loot_interact_tests). CLI already runs `ProjectConfig::validate()`
  (validate.rs:1577); no closed vocabulary exists for inventory/shop/item events (only lobby). `SceneStateParams` is a
  derive-SystemParam with ~38 fields (the 16 cap applies to the executor's top-level params only).
  Container->container transfer needs two `&mut Inventory` from one query: clone-and-swap with sequential write-back
  sidesteps `get_many_mut`.

- S2a code review (2026-11-14, `feature/item_transaction_primitive`): primitive = `capabilities/item_txn.rs` (`plan` on
  owned `TxnLedger`), glue = `runtime/scene_manager/item_actions.rs`. Stock is `MerchantStockState` (BTreeMap<Entity,..>,
  lookup-only; Entity key makes the LoadScene clear mere hygiene). **Trap found:** refreshing shop rows by pushing
  `Action::OpenShop` to the FIFO queue re-opens a shop that a later action in the same batch closed (`[BuyItem, CloseShop]`,
  pause `entry_actions` CloseShop) — refresh in place instead. TakeAll and AddItem's slot path still call
  add_to_slots directly (deliberate); coin->stat logic exists in 3 copies (credit_coins, transfer, TakeAll). After S2a a
  coin item given to the player becomes a stat, so `RemoveItem`/`requires_item` of a coin can never succeed (asymmetry).

- S3 plan review (2026-11-14, at 60e28b0, `item_use.md`): (a) "matched this frame" default-backed suppression breaks the
  "state-scoped rule = gate" pattern (demo's buy_item rules are in-state "playing"; in any other state a click would run the
  default); recommended a STATIC "bound anywhere in state_machine.ron => designer owns it" set instead (no interpreter change).
  (b) CLI `check_ui_trigger_reachability` makes a missing `buy_item:{key}` rule a HARD error (validate.rs ~3063) and
  `collect_reachable_ui_triggers` must learn `use_item:{key}`. (c) executor drains the queue until empty in one frame, so a
  `UseItem` inside `use.actions` (with an `AddItem` of the same item) is an infinite loop / frame hang: forbid it.
  (d) `TxnOp::Take{count:0}` always succeeds, so `consume: 0` = free infinite use unless guarded. (e) one bad action in
  items.ron fails the whole file -> `LoadedItemCatalog(None)` -> coins become bag items, icons gone (error! only).
  (f) `every_edge_triggered_input_system_reads_the_pause_clock` forces a SimClock read on the slot click system.
  (g) no executor-side `{target}` rewrite exists; effects pushed by the executor never get one.

- S3 code review (2026-11-15, `feature/item_use`, uncommitted on 543017a): `ActionQueue::push_front_all` is the one
  executor-only exception to FIFO (pub, enforced by doc only; crate CLAUDE.md:21 + docs/30:673 still say "push order is
  execution order"). No composite Action variants exist, so UseItem is the ONLY synchronous re-entry into the executor
  drain; filtering nested UseItem makes a same-frame loop impossible. Cross-frame loop still possible
  (`item.used:x -> UseItem(x)` with `consume: 0` = one use per frame forever; not validated). `BoundPanelEvents` is
  static (global_on + state on:), rebuilt on `LoadedStateMachine.is_changed()`; that resource is only ever written via
  `commands.insert_resource` in project_loader, read via `Res`, so no spurious rebuilds. Default pusher acts on ANY
  UiEvent with the prefix (authored Buttons/key bindings too), but CLI forward check still hard-errors an authored
  `use_item:`/`buy_item:` Button without a rule (false positive). Reverse check only marks `use_item:{key}` reachable for
  items WITH `use:` though slots emit it for every item. Slot Buttons (FocusPolicy::Block) take Pressed/Hovered from the
  panel root under the cursor; harmless (targeting checks ANY Pressed). items.ron use actions are added to validate's
  all_actions AFTER discover_extra_scenes (a LoadScene in a use list doesn't load that scene for validation).

- S2b plan review (2026-11-15, at 29cce26, `player_inventory_slots.md`): nothing resets `PlayerInventory` on LoadScene
  (persists) or project load; exactly one ProjectConfig loads per app lifetime (`project_loader.rs` tail comment), so a
  project-load insert = boot sizing, no "project switch". `check_project_loaded` has TWO resource-insert branches
  (no-external-files phase 1 and phase 2) plus a shared tail (where `max_frame_delta` applies): new per-project
  resources go in the tail. `ProjectConfig` is `derive(Default)` and ~45 test literals use `..Default::default()`,
  so a nested def's Default must be hand-written to the serde default (else 0 slots in tests). Dead slots already
  ignore clicks structurally (`slots.get(idx)` is None past `len`); no system styles slot hover (vacuous "no
  highlight"). `ironhold query` has no project-config subcommand. Only `3rd_person_game_demo` has an InventoryPanel
  (5x4 = `default_inv_columns`*`rows` = DEFAULT_PLAYER_SLOTS, an unpinned coincidence).

- S2b code review (2026-11-15, `feature/player_inventory_slots`, uncommitted on c22b1c2): `PlayerInventory` has exactly
  two writers (loader-tail insert, `SceneStateParams` ResMut in the executor) + Res readers (interactable, UI, slot click,
  `SceneV2Params`); `spawn_scene_v2` takes no `SceneStateParams`, so no Res/ResMut conflict. Only the bag panel spawns
  `InventorySlotMarker` (containers/shop do not), so the faded writer is safe; the only other BackgroundColor writer on
  Buttons (`button_system`) filters `&UiAction`, which slots lack. Implicit invariant `slots.len() == max_slots`
  (pub fields): UI icons/labels key off `slots.get`, the click guard off `max_slots` - a future `SetInventorySize`
  must keep them equal (`resize` already truncates, silently dropping items).

- S4 plan review (2026-11-15, at a65e696, `item_sell_destroy.md`): `button_system` (lib.rs:634) has NO SimClock gate
  (shop Buy buttons fire while paused; only the inspector gate). The default-backed prefix list is now hand-copied in 4
  places (BoundPanelEvents, default_panel_actions_system, CLI `item_event_rule_warnings::is_panel_click` ~1000, forward
  check `default_backed` ~3142, reverse `collect_reachable_ui_triggers` ~3316): one shared const slice is the fix.
  A count snapshotted by the default pusher (Execute, before the executor) can go stale vs actions the interpreters
  queued ahead of it in the same drain -> prefer an executor-resolved "all". `ShopEntry` has no Default and test literals
  write `sell_price: 0` (item_transaction_tests.rs:111). Every Close path of a panel differs (Close*, Toggle*, LoadScene,
  pause): derive "menu must close" from state (open_panels/SimClock/bag), never enumerate actions.

**How to apply:** any new item-moving action must go through the primitive (once built) and emit a failure event
instead of a success event on failure. See [[inventory-panel-triplication]], [[panel-input-blocking]],
[[event-pipeline-intent-layer]], [[stat-ownership-global-vs-instance]].

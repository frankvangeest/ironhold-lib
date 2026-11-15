# Feature: Selling and destroying items (item system S4)

_Status: In Progress (plan-reviewed 2026-11-15; Frank's answers to Q3, Q5, Q6 folded in)_
_Planned at: `a65e696` (2026-11-15)_

Slice S4 of `planning/features/item_system_review.md`. Builds on S2a (the item transaction primitive,
`planning/features/done/item_transaction_primitive.md`), S3 (`UseItem` and default-backed panel clicks,
`planning/features/done/item_use.md`) and S2b (bag size from data). NOT part of this slice: the vendor buy-back list (S7, Icebox),
item use conditions (backlog), hover tooltips, action bar item slots (S5/S6), gamepad/keyboard bag navigation.

## What
A designer can make an item sellable (`sell_price` in `items.ron`) and protect story items from being thrown away (`destroyable: false`).
The player clicks an item in the bag and gets a small menu: **Use**, **Sell** (only while a shop is open), **Destroy** (asks "are you sure"
first). Two new actions, `SellItem` and `DestroyItem`, do the same from rules or a bar slot. Today a full bag has no way out except using
the items, and a shop can only be bought from.

## Why
Closes the player-facing loop (buy, use, sell, destroy). Sell and destroy are two more verbs on the S2a primitive (`[Take, Stat(+price)]`
and `[Take]`), and S3 already made panel clicks default-backed, so no per-item rules are needed. The slot menu is also the place where S6
("assign to the action bar") and later verbs will go.

## Decisions already taken (item system review, Frank, 2026-11-14 and S3 plan)
- Sell price is an **item** property; no `sell_price` means unsellable (covers quest and key items). `ShopEntry.sell_price` becomes an
  optional per-merchant override. Selling needs an open shop and pays in that merchant's `currency_stat`.
- `destroyable` defaults to `true`; key and quest items set `false`. Destroy always asks for confirmation **in the panel UI**; the
  `DestroyItem` action itself never asks (a designer-fired action must not block on UI).
- Click menu plus right-click shortcut (decision 9): **left click or tap opens the menu**, right click stays "Use" (the S3 shortcut).
- A failure never emits a success event; success events keep the `item.*:{item}` specific form.

## Approach

### 1. Schema (additive; no `schema_version` bump)
- `ItemDef.sell_price: Option<u32>` (default `None`) and `ItemDef.destroyable: bool` (default `true`). `deny_unknown_fields` stays.
- `ShopEntry.sell_price: u32 -> Option<u32>`. Every authored `sell_price: 5` still parses (`implicit_some`, `schema/ron_loader.rs`).
  Needs `#[serde(default)]` or an omitted field will not parse. **`0` means "this merchant does not buy it"** (Q1, both reviewers): the demo's
  `old_key` and the docs' `key_iron` already write `sell_price: 0` that way, and a "sell for 0" row is just Destroy. `ItemDef.sell_price: 0` means
  the same (unsellable). No strict warning on 0. A merchant that refuses whole categories (`buys_tags`) is logged in the backlog, not built.
- Price used when selling: `merchant ShopEntry.sell_price` (if that merchant stocks the item and sets it) else `ItemDef.sell_price`;
  neither means unsellable. The merchant buys any item with a price, whether or not it stocks it (WoW model).
- `ItemDef.destroyable` uses a named default fn (schema-defaults rule). Test literals that break: `ShopEntry` in `item_transaction_tests.rs:111`,
  `ItemDef` in `item_txn.rs:310` and two in `spawn_tests.rs`.
- `Action::SellItem { item_key, count (default 1), all (default false) }` and `Action::DestroyItem { .. same .. }`, struct variants like
  `TransferItem`. `all: true` is resolved **in the executor** with `count_item` at run time (never read earlier from the bag: the default pusher
  runs after the interpreters, so a count taken before queued work would go stale), which also makes a rule that replaces the built-in able to
  reproduce it (`DestroyItem(item_key: "x", all: true)`). `count: 0` without `all` is refused at runtime and is a `validate` error. Price math
  uses `checked_mul`/u64. Catalog keys, no substitution. Exhaustive matches to update: `Action::is_presentation_only` (`actions.rs:523`, no `_`
  arm), `rewrite_self`/`rewrite_target` (`action_substitution.rs:119/255`), the executor arm, `query.rs:689`, `validate.rs:1393` (catalog-key arm).

### 2. Executor (`item_actions.rs`, through `item_txn::plan`)
- `sell_item(item_key, count)`: refused (nothing changes, `item.sell_refused` + `item.sell_refused:{item}`) when no shop is open, the item
  is unknown, has no price at this merchant, is not held in that count, or paying out would pass the currency stat's maximum
  (`TxnError::NoRoom` on the stat). Otherwise `plan([Take{Player,item,count}, Stat{currency, +price*count}])`, write back, emit
  `item.sold:{item}`, say "Sold {name} for {n} {currency}". The currency stat resolves with `resolve_currency` (player's own stat first,
  then the shared pool), the same rule `BuyItem` uses.
- `destroy_item(item_key, count)`: refused (`item.destroy_refused` + `:{item}`) when unknown, `destroyable: false`, or not held in that
  count; otherwise `plan([Take])`, emit `item.destroyed:{item}`. No stat change, no message by default (a rule can add one).
- A missing currency stat refuses the sale with a `warn!` (do not copy BuyItem's fake `max: 0` ledger, which would report "no room"). The
  built-in messages use a readable currency name, not the raw stat key, and a refused sell or destroy says why ("Can't sell that here",
  "Can't carry more gold"). In local co-op the bag is shared and the primary player's stat is paid, as for Buy (documented).
- Sold items are gone: the merchant's `MerchantStock` is **not** increased (buy-back is S7). Document it.
- `Unsellable`/`Undestroyable` are checked as data rules before `plan` (the executor knows the catalog); they are added to `TxnError` only
  if a test shows the primitive needs them.
- Refresh: after a sale the shop rows do not change (stock is untouched); the bag UI refreshes through the normal `inventory_ui_system`.

### 3. Default-backed events and the slot menu
- New panel events, default-backed exactly like `buy_item:`/`use_item:` (a binding in `global_on` or any state's `on:` owns the event
  everywhere; `BoundPanelEvents` learns the prefixes): `ui.button_pressed:sell_item:{key}` (one unit), `ui.button_pressed:sell_stack:{key}`
  (the clicked stack) and `ui.button_pressed:destroy_item:{key}` (the clicked stack, after the confirm).
  `default_panel_actions_system` pushes `SellItem`/`DestroyItem` with `count` = the **clicked stack's quantity** stored in `SlotMenuState` at
  open time for `sell_stack`/`destroy_item` (and 1 for `sell_item`). The action's own `all: true` stays "every unit across all stacks" for
  designers; the menu never uses it. If queued work changed the bag in between, `Take` fails and the action is refused with nothing lost.
  Which stack `Take` removes from does not matter (same item, same total); only the quantity is the clicked stack's. A rule that replaces
  `destroy_item:` chooses its own count; docs state what each built-in uses.
  The prefixes live in **one** `pub const DEFAULT_BACKED_PREFIXES` in core, used by all five hand-copied sites: `BoundPanelEvents`
  (`panel_defaults.rs:33`), the default system (`:59-63`), `validate.rs` `is_panel_click` (`:1000`, the "transition/behavior binding runs both"
  warning), the forward `default_backed` check (`:3142`, a hard `unreachable_trigger` error today, not a strict warning) and the reverse check (`:3316`).
- **Menu:** one owning system extends `inventory_slot_click_system` (it stays the single `UiEvent` writer in its `EmitSet::Ui` chain, so the
  opening click cannot also count as an outside click in the same frame). Left click or tap on a non-empty slot opens the menu; right click
  still emits `use_item:{key}`. State is `SlotMenuState` **inside `LoadedInventoryUi`** (reset whole on a Replace load): item key, total count and
  slot index at open time, plus a `confirming_destroy` flag; opening emits no event.
- **Menu node:** one overlay per `InventoryPanel`, an **absolute child of the panel root** placed from the slot index (columns, slot size, gap,
  padding; no cursor or window pixel math, so no `UiScale` trap), `GlobalZIndex` above 99, `FocusPolicy::Block`, flips up or left near the
  viewport edge, hides with the panel, tagged `LevelEntity` like the panel. Rows are plain `Button`s with a `SlotMenuRow` marker, **not**
  `UiAction::Trigger` (`button_system` has no SimClock gate and the Destroy row must emit nothing). Minimum row height about 40 px
  (touch). Rows are rewritten with guarded writes; the system returns early when closed and nothing was pressed.
- **Rows:** a header with the item name; **Use** (item has `use:`); **Sell {price} {currency}** (one unit) and **Sell N** (the clicked stack) when N > 1
  (shop open and sellable); **Destroy**. With no verb at all (a key item) one dimmed row, "Cannot be sold or destroyed". **Sell N** and
  **Destroy** both go through a Yes/No confirm that names the quantity ("Destroy 10 Health Potion?"); a single unit sells at once. The
  confirm is UI: an override on the event still gets it, while `DestroyItem` fired by a rule or bar slot never asks.
- **Closing is derived from state every frame, not from a list of actions:** the menu closes when the inventory panel is not open
  (`ToggleInventory`, the panel's own close button, `LoadScene`), when `!sim.is_running()` (the pause menu), when the shop open state changes
  (a `CloseShop` from walking past the merchant would otherwise leave a stale Sell row), when the slot's item key or total count no longer
  matches what was stored at open time, on a click outside, and on a verb chosen. **Escape is not used by S4** (Frank, 2026-11-15: "Escape closes the topmost
  window if one is open, otherwise pauses" is a general engine rule, backlog item "Escape closes the topmost window before it pauses"; until it
  lands the pause menu closing the slot menu is enough, and the menu must register as a closable layer then). Mouse/touch only like the bag today;
  gamepad and keyboard players use `SellItem`/`DestroyItem` rules or bar slots (documented).
- It is ignored while `InspectorEnabled`, like `inventory_slot_click_system`. New systems join `EmitSet::Ui` explicitly and
  `schedule_order_tests` stays green. Plain ASCII labels; text size follows the panel's `font_size`.

### 4. Shared panel chrome: not in S4
Both reviewers noted the menu has no header block or close button, so the helper would not shrink S4, and it edits the same three arms of
`scene_loader.rs` the menu work touches. **Recommendation (Q5): keep "Extract a shared panel-chrome helper" as its own backlog item, done on
its own small branch before or after S4.** If Frank wants it inside S4 it is a separate first commit and the baselines must not move.

### 5. CLI (`ironhold validate`, `query`)
- `query actions` lists `SellItem` and `DestroyItem`.
- Errors: `SellItem`/`DestroyItem` naming an item that is not in the catalog.
- Errors also: `count: 0` without `all: true`.
- Warnings: `SellItem` on an item with no `sell_price` and no merchant price anywhere (never succeeds); `DestroyItem` on a `destroyable: false`
  item; `sell_price` on a currency item.
- Warning (not strict): an item named by an interactable's `requires_item` that has a `sell_price` (a sellable key soft-locks the quest).
  `--strict`: such an item that is `destroyable`; a merchant `sell_price` above its own `buy_price` for the same item (buy-sell gold exploit).
- `sell_item:`/`sell_stack:`/`destroy_item:` triggers authored on a button reach the built-in: no `unreachable_trigger` **error** (forward
  check) and no reverse-check orphan, as for `buy_item:` (see the `DEFAULT_BACKED_PREFIXES` note above).
- `collect_reachable_ui_triggers` counts the three new triggers as reachable when an `InventoryPanel` exists, so the override example is not
  an orphan.

### 6. Demo (`3rd_person_game_demo`)
`health_potion` / `mana_potion`: `sell_price: 5`; `iron_sword`: `sell_price: 30`; `old_key`: `destroyable: false`, no `sell_price` (the
merchant's `sell_price: 0` on it keeps its meaning). Update the HUD hint to "click a bag item for Use / Sell / Destroy" and every stale
"click to use" text: `items.ron:13`, `docs/20` (~5009 and ~5118, "a later version opens a menu" becomes "now"), `state_machine.ron:178`.
One example rule in `state_machine.ron` (`item.sold:iron_sword` or an override of `destroy_item:` with `all: true`).

### Events
New: `item.sold:{item}`, `item.sell_refused` and `item.sell_refused:{item}`, `item.destroyed:{item}`, `item.destroy_refused` and
`item.destroy_refused:{item}`. (A reason form is the existing follow-up for `use_failed`; do both together later.)

### RON authoring examples
```ron
// items.ron
"health_potion": ( display_name: "Health Potion", max_stack: 10, sell_price: 5, use: ( do_actions: [ ModifyStat(key: "player_health", delta: 40.0) ] ) ),
"old_key":       ( display_name: "Old Key", stackable: false, destroyable: false ),   // no sell_price: cannot be sold either

// prefabs.ron: this merchant pays 50% more for swords than the item's own price
(item_key: "iron_sword", buy_price: 80, sell_price: 45),

// state_machine.ron
( event: "item.sold:iron_sword",            do_actions: [ ShowFloatingText(entity: "player_01", text: "Pleasure doing business") ] ),
( event: "item.destroy_refused:old_key",    do_actions: [ ShowFloatingText(entity: "player_01", text: "I should keep this") ] ),
( event: "ui.button_pressed:destroy_item:iron_sword", do_actions: [ DestroyItem(item_key: "iron_sword", all: true), PlaySound(key: "break") ] ),  // replaces the default
// from a bar slot or a quest rule
SellItem(item_key: "gem", count: 3)
```

## Tasks
- [ ] Failing tests first (red against today's code): sell takes the item and pays `price * count` in the merchant's currency; refused (changes
      nothing) with no shop open / no price / not held / gold at its maximum; merchant `sell_price` override beats the item price; a merchant that
      does not stock the item still buys it; stock is not increased by a sale; destroy removes the count, refused for `destroyable: false` and
      not-held; `item.sold` / `item.destroyed` only on success; the three default events run unless a rule is bound anywhere (no double sell);
      `sell_stack` sells the clicked stack's quantity only; the menu opens on left click, right click still uses, the menu rows follow shop open /
      `use:` / `destroyable` / stack size, Destroy needs the Yes step, outside click / Escape / `LoadScene` / pause close it; a paused clock
      and the inspector gate; `ShopEntry.sell_price` parse (`5`, omitted)
- [ ] Schema (`ItemDef`, `ShopEntry`, the two actions, every exhaustive match)
- [ ] `item_actions::sell_item` / `destroy_item`, default-backed events, `BoundPanelEvents`
- [ ] Slot menu overlay + `SlotMenuState` + systems in `EmitSet::Ui`, `schedule_order_tests`
- [ ] `ironhold validate` / `query` changes with `validate_cross_file` tests both directions
- [ ] Demo, docs: `docs/20` (`ItemDef` rows, `ShopEntry.sell_price` type change, "Selling and destroying" section, the menu and what closes it,
      events table, gamepad note, "no buy-back"), `docs/30` (events, actions), `docs/15` (validate rows), `STATUS.md`,
      `runtime/scene_manager/CLAUDE.md`, `capabilities/CLAUDE.md` (the menu)
- [ ] Reviews, full core test loop, `cargo check -p ironhold_cli`, `cargo test -p ironhold_cli`, `ron_lint`/`ron_validation`

## Out of scope
Buy-back (S7), a quantity picker (the menu offers one or all), `left_click: Menu | Use`, `MerchantDef.buys_tags` (a merchant that refuses categories), the shared panel-chrome helper (own item), item use conditions, tooltips showing the price, drag to sell or drag to the
bar (S6), selling to a container (`TransferItem` does that), per-player gold, reputation prices, gamepad/keyboard bag navigation.

## Open questions
Settled by both reviewers (2026-11-15): **Q1** `0` means "not bought here" (above). **Q2** left click opens the menu, right click stays Use;
`InventoryPanelDef.left_click: Menu | Use` is logged in the backlog, not built; S3's left-click tests change (S3 is unreleased). **Q4** one
branch, two commits (actions + schema + defaults + validate, then the menu), one playtest; each commit compiles and passes the tests.

Decided by Frank (2026-11-15): **Q3** Sell and Destroy act on the clicked stack's quantity (UX reviewer's recommendation), labels show N.
**Q5** the shared panel-chrome helper stays its own backlog item, not in S4. **Q6** Escape closes the topmost window if one is open, otherwise
pauses; a general input rule (it also covers the WoW-style "first press leaves camera orbit or a captured cursor, second opens the menu"),
split out of S4 into its own item, so S4 does not bind Escape.

## Acceptance criteria
- Given a health potion (`sell_price: 5`) and an open shop, when the player picks Sell in the menu, then one potion leaves the bag, the player
  gains 5 of the merchant's currency, `item.sold:health_potion` fires and the shop stock is unchanged.
- Given no shop open, then the menu has no Sell row and `SellItem` is refused with `item.sell_refused`.
- Given `old_key` (`destroyable: false`, no `sell_price`), then the menu offers neither Sell nor Destroy and both actions are refused.
- Given Destroy chosen on an item, then nothing happens until Yes; No or a click outside leaves the bag unchanged.
- Given a bag with two stacks of 10 health potions and the second one clicked, then "Sell 10" sells 10, not 20.
- Given a rule on `ui.button_pressed:destroy_item:iron_sword` in any state, then only the rule runs.
- A failed sell or destroy never emits `item.sold` / `item.destroyed` and changes no bag, stat or stock.
- `ironhold validate` rejects an unknown item key in `SellItem`/`DestroyItem` and `--strict` flags a destroyable or sellable `requires_item` key.

## Implementation notes (2026-11-15, from the step-4 reviews)
- **`key_item_sellable` is `--strict` only**, not a plain warning as section 5 says: `ironhold validate` has errors and `--strict` warnings, no middle level.
- **The menu hangs under the clicked slot** (re-parented) instead of being an absolute child of the panel, and `slot_menu_sync_system` runs in `PostExecute`
  (it sees the bag after the frame's actions). "Flips near the viewport edge" became "opens towards the middle of the panel" (no window maths); a panel at the
  screen edge can still push it off screen (backlog).
- **The Yes of a Yes/No is ignored for 350 ms** after the question appears (`CONFIRM_ARM_MS`, real clock): the question replaces the verb rows in the same
  spot, so the second click of a double-click on Destroy would land on Yes.
- **`LoadedInventoryUi.menu_action` records the verb** (`sell_stack:` / `destroy_item:`), the item and the clicked stack's quantity, so an authored button on the same frame cannot take it.
- **The menu offers Destroy only for an item the catalog knows and marks destroyable** (no `items.ron` = no Destroy row), matching what `DestroyItem` accepts.
- **Which stack `Take` empties is not the clicked one** (`remove_from_slots` walks from the last slot); the quantity is the clicked stack's, the total is right.
  Making `Take` slot-aware needs a slot on the action or a menu-only op: backlog.
- The built-in messages name the stat key as it is and cannot be turned off (same open gap as "Bought X!"): backlog items extended.
- Not covered by a test (needs layout or a real pointer): the double-click geometry, the menu position on screen and touch. Playtest: see the checklist given with the review.
- **Added at the playtest (Frank, 2026-11-15): "Sell all N" and "Destroy all N"** for an item the bag holds more of than the clicked stack (a non-stackable
  sword over several slots, or a second stack). New default-backed events `sell_all:{key}` and `destroy_all:{key}` (plain `all: true`, no menu quantity); both ask
  Yes/No with the total. This replaces the "whole-item selling across stacks as its own verb" follow-up.

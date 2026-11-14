# Feature: Item transaction primitive (item system S2a)

_Status: Ready_
_Planned at: `9fa2ef3` (2026-11-14)_
_Plan-reviewed 2026-11-14: system-architect and ux-gamedesigner-reviewer both returned "needs more design work"; Frank's
answers to the four blocking questions and the reviewers' remaining points are folded in below._

Slice S2a of `planning/features/item_system_review.md` (the architect's item system review; Frank's answers to its nine
questions are recorded there). S1 (panel open-state) is done. **S2b (the `player_inventory` project field) is split off
as its own item** (see "Split off"). S3-S6 (use, sell, destroy, action bar item slots) build on this slice and are NOT
part of it.

## What
Every action that moves items or gold today does its own ad-hoc add/remove, and three of them lose things: `BuyItem`
takes the gold before checking the item fits (a full inventory costs the gold and gives nothing), `TransferItem`
removes items before checking they fit (overflow is destroyed, and `inventory.transferred` fires even when nothing
arrived), and a shop's `stock_count` is never decremented (a `stock_count: 1` key sells without limit). This slice adds
one all-or-nothing item transaction used by those actions, makes merchant stock real runtime state that refills on
scene load, routes coin items to their currency stat in every action, gives every buy outcome a defined event and a
built-in message, and lets `RemoveItem` be all-or-nothing. For a designer: buying, transferring and removing items
fail cleanly with an event they can react to, and shops can sell out.

## Why
Playtests on 2026-11-14 hit the key duplication (stock never decremented) and the item-loss class of bug three times in
three different actions. S3 (use), S4 (sell, destroy) and S5/S6 (action bar) all need "take N of X, fail if not enough"
and "give N of X, fail if no room" with defined failure events; building them on today's per-action code would repeat
the bug four more times. No test covers `BuyItem`, `TransferItem`, `AddItem` or `RemoveItem` today (only
`TakeAllFromContainer`, in `corpse_loot_interact_tests.rs`), so this slice's tests are the whole regression net.

## Decisions (Frank, 2026-11-14)
1. **Merchant stock resets on scene load** (like chests and corpses). Not persistent across scenes; no `RestockMerchant`
   action; no save system. (This replaces the earlier reading of the "3: false" answer as "persist"; persisting would
   strand the demo's `old_key` quest after a scene reload.)
2. **Built-in feedback for all four buy outcomes**, shown over the primary player: bought, not enough `<currency>`,
   sold out, bag is full. A designer can replace them once S3's default-backed-event mechanism lands; until then they
   cannot be turned off, and the docs say so. No demo rule duplicates them.
3. **Events as generic + item-specific pairs**, plus `shop.no_room:{item}` and `inventory.transfer_failed`.
4. **`RemoveItem` gains an optional `all_or_nothing: bool`** (default `false` = today's behaviour).

From the reviews, adopted without a separate question: `player_inventory_slots` is split off (S2b); sold-out rows stay
in the list, dimmed, with "Sold out" in place of the count, and stay clickable; coin items sent to a non-player
inventory stay items (the stat is credited only when the destination is the player); "whose gold" is the primary
player's own stat if its prefab has it, otherwise the shared pool.

## Approach
Details and reasoning: `planning/features/item_system_review.md` sections 1-3. The parts that are S2a:

1. **Pure transaction function on an owned ledger** (no ECS types inside, unit-testable without an `App`), in
   `capabilities/inventory.rs` or a new `capabilities/item_txn.rs`. The executor gathers an owned `TxnLedger` (cloned
   slot vectors with their max slots per inventory reference, cloned stat values per stat reference, remaining stock
   per item), calls `plan(&ops, ledger, &catalog) -> Result<TxnLedger, TxnError>`, and on success writes each touched
   entry back one at a time with `get_mut` (no `get_many_mut`, no several `&mut` borrows held across the call, which is
   what a container-to-container `TransferItem` would otherwise need). Ops apply in order on a `Vec`; no hash-map
   iteration.
   - `TxnOp::{Take{inv,item,count}, Give{inv,item,count}, Stat{target,key,delta}, Stock{item,delta}}`, with
     `StatRef::{Player(spawn_id), Global}` resolved before planning. The stat floor and ceiling are the stat's own
     `min`/`max`, not 0; coins that would pass the max are `NoRoom` and stay in the source (`apply_delta` would
     silently clamp them away). Write-back keeps `apply_delta`'s existing behaviour of resetting the regen cooldown
     when a stat drops.
   - `TxnError::{NotEnoughItems, NoRoom, InsufficientFunds, SoldOut, UnknownItem}`. `UnknownItem` only when an item
     catalog is loaded and lacks the key; a catalog-less project keeps today's "stackable to 99" behaviour with a
     `warn!`. (`Unsellable`/`Undestroyable` arrive with S4.)
   - Runs inside `action_executor_system` (Update), like every item action today. The new state lives in
     `SceneStateParams` (the derived bundle, no top-level parameter limit issue).
2. **`BuyItem`** = `[Stock(-1), Stat(-price), Give(player, 1)]` in one transaction. Order matters: a broke player
   clicking a sold-out item hears "sold out", not "not enough gold". Failure leaves gold, stock and items untouched.
3. **`TransferItem`** moves what fits per stack and leaves the rest in the source; `inventory.full:{to}` for leftovers;
   `inventory.transferred:*` only when something moved (Frank, earlier decision 7); `inventory.transfer_failed` when
   nothing moved. The old behaviour (destroy overflow, always emit) is a bug, not a contract.
4. **`AddItem` / `RemoveItem`** keep their contracts (partial add + `inventory.full:*`; remove what is there) but use
   the primitive. A coin item (`ItemDef.currency_stat`) given to the player credits the stat instead of occupying a
   slot, in every action, and still emits `inventory.added:player:{coin}:{n}` (as `TakeAll` does today); a coin whose
   stat does not exist stays an ordinary item with a `warn!` (in `TakeAll` it stays in the container). `RemoveItem`
   that removes nothing emits `inventory.remove_failed`; with `all_or_nothing: true` and fewer than `count`, nothing is
   removed and `inventory.remove_failed` fires.
5. **`TakeAllFromContainer`** is re-expressed as a loop of single-stack transactions with the same observable
   behaviour; its tests must stay green unchanged.
6. **One currency resolver, one primary player.** `resolve_cost_source` (private in `action_bar.rs:358`) moves into a
   shared helper (the player's own `StatMap` first, then `LoadedStats`) used by `BuyItem`, `AddItem`, `TakeAll`, the
   action bar **and the inventory gold footer** (`inventory_ui_system` reads `LoadedStats` only today and would
   otherwise disagree with the shop). "The player" is `primary_player_spawn_id`: the player with the lowest
   `PlayerIndex` (`PlayerInventory.player_spawn_id` is only set by the dynamic spawn arm, so it is `None` for a
   scene-placed player). The inventory is shared in co-op: items go to the shared bag, gold comes from player 1's own
   pool if it has the stat. The same player id is where the built-in messages appear. No shipped project changes
   behaviour (`gold` exists only in the global `stats.ron`).
7. **Merchant stock as a `MerchantStock` component on the merchant entity**, inserted at spawn from `MerchantDef.stock`
   (remaining per item; `None` = unlimited). It dies with the entity, so it refills on scene load and respawn, which
   makes the schema/doc claim true by construction. `OpenShop` and `BuyItem` read it, never the prefab catalog. The shop
   row for a sold-out item stays, the Buy button is dimmed, the count becomes "Sold out", and it stays clickable (a
   click emits the sold-out event). Labels refresh from a `PostExecute` system on a change marker, not by rebuilding
   rows. Fix the wrong schema comment (`schema/catalog.rs:1692`) and the docs.
8. **Built-in messages** reuse the existing floating-text path for all four buy outcomes: "Bought {item}", "Not enough
   {currency display name}" (replaces the hardcoded "gold"), "Sold out", "Bag is full". Plain ASCII only. No new
   message for the non-buy actions.

Schema impact: `Action::RemoveItem` gains `all_or_nothing: bool` (additive, `#[serde(default)]`; no `schema_version`
bump). `ironhold query actions` and `validate` must show it. `MerchantDef`/`ShopEntry` unchanged (`sell_price` is S4).

### Events (success events unchanged; a failure never emits a success event)
Kept exactly: `inventory.added:{inv}:{item}:{n}`, `inventory.removed:{inv}:{item}:{n}`, `inventory.full:{inv}`,
`item.bought:{item}`, `container.looted:{id}`, `inventory.transferred:{from}:{to}:{item}` (now only when something moved).
New or extended, generic form then the specific form (the `target.changed` / lobby precedent):
- `shop.sold_out` and `shop.sold_out:{item}`
- `shop.insufficient_funds` (unchanged) and `shop.insufficient_funds:{item}`
- `shop.no_room:{item}` (the bag could not hold a bought item; `inventory.full:player` is shared with loot and `AddItem`)
- `inventory.remove_failed:{inv}` and `inventory.remove_failed:{inv}:{item}`
- `inventory.transfer_failed:{from}:{to}:{item}`

### RON authoring examples
```ron
// react to a sold-out shop item (generic form: one rule covers every item)
( event: "shop.sold_out",            do_actions: [ PlaySound(key: "click") ] ),
( event: "shop.no_room:iron_sword",  do_actions: [ ShowFloatingText(entity: "player_01", text: "No room for the sword") ] ),

// quest turn-in that must not run its reward unless all three pelts were taken
( event: "entity.interacted:hunter", do_actions: [ RemoveItem(entity: "player", item_key: "wolf_pelt", count: 3, all_or_nothing: true) ] ),
( event: "inventory.removed:player:wolf_pelt:3", do_actions: [ AddItem(entity: "player", item_key: "gold_coin", count: 50) ] ),

// prefab merchant: unchanged; stock_count now really runs down and refills on scene load
merchant: ( currency_stat: "gold", stock: [ ( item_key: "old_key", price: 50, stock_count: 1 ) ] ),
```
Existing projects keep working: merchants without `stock_count` stay unlimited, and the existing per-item
`ui.button_pressed:buy_item:{key}` rules still drive `BuyItem` unchanged.

## Tasks
- [ ] Failing tests first (red against today's code): buy into a full inventory keeps gold and emits no `item.bought`;
      buy a `stock_count: 1` item twice (second fails with `shop.sold_out`); a sold-out item that is also unaffordable
      reports sold out; transfer into a full inventory leaves the rest in the source and emits `inventory.full`;
      container-to-container transfer; a coin item via `AddItem` credits the stat; coins past the stat's max stay put;
      `RemoveItem` of 0 and `all_or_nothing` with too few emit `inventory.remove_failed`; stock refills after
      `LoadScene` and is not shared with a same-id merchant of a different prefab; the footer agrees with the shop when
      the player has its own gold stat; the built-in message reaches a scene-placed player
- [ ] Pure transaction function with unit tests (all-or-nothing, ordering, stack merge, freeing and filling a slot)
- [ ] `BuyItem`, `TransferItem`, `AddItem`, `RemoveItem`, `TakeAllFromContainer` on the primitive
- [ ] Shared currency resolver + `primary_player_spawn_id`; footer, shop, bar and loot all use them
- [ ] `MerchantStock` component; shop label and sold-out row; built-in messages; the five event families
- [ ] `RemoveItem.all_or_nothing` in the schema, CLI `query`/`validate`; fix the `actions.rs` doc comments (~`:377-428`)
- [ ] Docs: `docs/20_data_formats.md` (`stock_count` row, `currency_stat` row, the items/merchant "v1 scope" blocks,
      pipeline events table incl. `item.bought`/`shop.*`/`container.looted`), `docs/30` (actions `BuyItem`,
      `TakeAllFromContainer`, `OpenContainer`, `CloseContainer`; the new events; the failure contract), a "what survives
      a scene load" table (player bag and gold yes; chest/corpse contents and merchant stock no), `docs/STATUS.md`
      "Shops and inventory" row, `capabilities/` and `runtime/scene_manager/` `CLAUDE.md`, stale text in the demo
      `stats.ron` and `schema/catalog.rs`
- [ ] Demo playtest aid (before step 4, so `ron_lint` covers it): a debug button that runs `AddItem` with 20
      non-stackable swords so the full-bag buy is reachable; no rule that duplicates the built-in messages
- [ ] Reviews, full core test loop, `cargo check -p ironhold_cli`

## Split off
**S2b: `player_inventory` project field** (Frank, decision 6 of the item review: project-level, default 20). Its own
backlog item and plan-review: `ProjectConfig.player_inventory: (max_slots: N)` (struct form, leaves room for starting
items) replacing the four hardcoded `resize(20)` sites; `PlayerInventory::default()` sized from one shared constant;
inserted at both resource-insert points in the project loader (`project_loader.rs:119-160` and phase 2); clamp to at
least 1 and range-check in `ProjectConfig::validate()`; `ironhold validate` checks the field against every scene's
`InventoryPanel` (`columns * rows`, nested panels via `walk_ui_nodes`) in both directions; dead slots rendered dimmed;
shrinking below occupied slots refuses and warns. In S2a the four `resize(20)` sites only share one constant.

## Out of scope (state in the docs)
Selling and buy-back (S4/S7), restock timers, per-player prices and reputation discounts, confirmation for expensive
buys, buying more than one at a time, greying out unaffordable items, hover tooltips and item descriptions, per-player
inventories in co-op (one shared bag), saving across page reloads, starting inventory, a "new game" reset of the bag.

## Open questions
None blocking. (Decide during implementation: the exact built-in wording; `inventory.transferred` gaining a `:{n}`
form for partial moves is polish and may be logged instead.)

## Acceptance criteria
- Given a full inventory, when the player buys an item, then no gold is spent, the stock is unchanged, `item.bought`
  does not fire, `shop.no_room:{item}` fires and "Bag is full" is shown.
- Given `stock_count: 1`, when the player buys it, then the row shows "Sold out", and a second buy emits `shop.sold_out`
  and `shop.sold_out:{item}` with gold untouched, until the next scene load, after which the stock is back.
- Given a sold-out item and a player without enough gold, then the failure reported is sold out.
- Given a container to player transfer with 1 free slot and 3 items, then what fits arrives, the rest stays in the
  container, `inventory.full:{to}` fires, and `inventory.transferred:*` fires only for what moved.
- Given a coin item, when `AddItem` gives it to the player, then the currency stat rises, no slot is used and
  `inventory.added:player:{coin}:{n}` fires.
- Given `RemoveItem(count: 3, all_or_nothing: true)` and 1 item held, then nothing is removed and
  `inventory.remove_failed` fires.
- The gold footer, the shop and the action bar cost agree on whose gold is spent.
- Existing `TakeAllFromContainer`, shop and container tests pass unchanged.

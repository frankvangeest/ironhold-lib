# Feature: Item system review (inventory, loot, shops, action bars, consumables, key items, currency as one system)

_Status: Draft_
_Planned at: `5fd373c` (2026-11-14)_

Design review only (system-architect, read-only, pre-implementation). It maps where item state lives
today, proposes one target model, and slices the work into backlog items. Nothing here is decided until
Frank answers the open questions at the end; each slice still needs its own plan (and plan-review where
marked) before code.

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| S0 | Land `feature/take_all_overflow` (already built, unmerged) | Queued | — |
| S1 | Panel open-state: idempotent set instead of a counter | Queued | — |
| S2 | Item transaction primitive + BuyItem/TransferItem/AddItem fixes + runtime merchant stock | Queued | — |
| S3 | Use items: `ItemDef.use` + `Action::UseItem` + use from the inventory panel | Queued | — |
| S4 | Sell and destroy (with confirmation), unsellable/key items | Queued | — |
| S5 | Item-reference action-bar slots (authored), count + greyed at 0 | Queued | — |
| S6 | Player-assignable bar slots (pick/place from inventory, clear) | Queued | — |
| S7 | Vendor buy-back list (optional) | Icebox | — |

## What

Today items are handled by six separately grown pieces: the global `PlayerInventory` resource, per-entity
`Inventory` components (chests, corpses), `MerchantDef` read straight from the prefab catalog, action-bar
slots that only know stat costs, `requires_item` on interactables, and currency as a global stat that only
one action converts into. This document defines one ownership model and one atomic "item transaction"
primitive that every item-moving action uses, then adds the missing player features (use consumables,
sell, destroy, item slots on the action bar) on top of it.

## Why

Evidence from 2026-11-14 playtests and reviews: every item bug found so far is the same bug shape,
"apply one side of a move, ignore whether the other side succeeded", repeated per action because each
action hand-rolls its own add/remove sequence. Fixing them one at a time (as `take_all_overflow` did) keeps
working, but the new features (sell, use, bar slots) would add four more hand-rolled sequences. One
primitive first makes every later slice small and testable without Bevy.

---

## 1. Current-state map (verified at `5fd373c`)

### Where item state lives

| State | Type / location | Lifetime | Notes |
|---|---|---|---|
| Player items | `PlayerInventory` resource, `capabilities/inventory.rs:9-28` (`slots: Vec<Option<ItemStack>>`, `max_slots`, `player_spawn_id`) | Persists across scenes (never reset in `LoadScene`) | One shared inventory for all local players. `max_slots` is lazily set to a hardcoded **20** on first add (`action_executor.rs:1221, 1305, 1595, 1699`), unrelated to the `InventoryPanel`'s `columns * rows` (`scene_loader.rs:2608-2620`): a 3x3 panel hides items in slots 9-19, a 6x4 panel shows four slots that can never fill. |
| Container items | `Inventory` component, `inventory.rs:85-95`; built from `PrefabDef.inventory` (`InventoryContainerDef`, `schema/catalog.rs:1656-1667`) in `entity_spawner.rs:75-91` | Per entity, gone on despawn/scene load | Corpses and chests are the same thing. |
| Merchant stock | `MerchantDef`/`ShopEntry`, `schema/catalog.rs:1673-1695`, on the **prefab catalog** | Immutable asset data | No runtime copy exists. `OpenShop` (`action_executor.rs:1367-1381`) and `BuyItem` (`:1536-1560`) re-read it from `prefab_catalog` each time, so `stock_count` can never decrease. Schema comment "restocks at scene load" (`catalog.rs:1693`) is wrong; `docs/20_data_formats.md:4980` correctly says "deduction not yet implemented". `ShopEntry.sell_price` is read by nothing (no sell action exists). |
| Currency | A stat in the global `LoadedStats` (`MerchantDef.currency_stat`, default `"gold"`); `ItemDef.currency_stat` (`schema/items.rs:66`) marks a coin item | Global, persists | Coin-to-stat conversion happens **only** in `TakeAllFromContainer` (`action_executor.rs:1687`). `AddItem`/`TransferItem` put a coin item into slots as an ordinary item. `BuyItem` reads `LoadedStats` only (`:1571-1585`), while the action bar's `resolve_cost_source` (`action_bar.rs:358`) prefers the player's own `StatMap`: two different currency-resolution rules. The gold footer label hardcodes `"Gold: "` (`inventory.rs:246`). |
| Key items | `InteractableDef.requires_item` (`catalog.rs:996`) -> `Interactable` (`interactable.rs:27`), checked with `has_item` (`inventory.rs:166`) at `interactable.rs:71-72` | — | Check only, never consumes; consuming takes an explicit `RemoveItem` in RON. Nothing stops selling/destroying a key item (there is no sell/destroy yet). |
| Panel state | `LoadedInventoryUi.panels_open: u8` + `active_merchant_id`, `inventory.rs:38-67`; `LoadedContainerUi.active_container`, `:70-79` | Reset to 0 on `LoadScene` (`action_executor.rs:54`) | A counter, gating `interactable_system` (`interactable.rs:52`), `collectible_system` (`collectible.rs:33`) and `tab_targeting_system` (`targeting.rs:308`). |
| Action-bar slots | `ActionSlotDef` (`schema/scene_v2.rs:1521`) -> `ActionSlotUi` (`action_bar.rs:56-80`) | Built at scene load from RON, immutable | Knows `do_actions` + an optional stat `cost`; no item concept, no runtime reassignment. |
| Item definitions | `ItemCatalog`/`ItemDef`, `schema/items.rs` (`LoadedItemCatalog`, `inventory.rs:32`) | Asset | `tags` exist (`"consumable"`, `"currency"` in the demo) but no code reads them; `weight` is unused. |

### Code paths that mutate item state (all in `runtime/scene_manager/action_executor.rs`)

| Action | Lines | What goes wrong today |
|---|---|---|
| `AddItem` | 1216-1252 | Partial add + `inventory.full:{entity}` (acceptable). Ignores `currency_stat`. |
| `RemoveItem` | 1253-1279 | Removes whatever is there; removing 0 emits nothing (no failure event). |
| `TransferItem` | 1280-1323 | Removes first, then ignores `add_to_slots`' result: **overflow is destroyed** (same bug as TakeAll), and `inventory.transferred` fires even when nothing arrived. Not in the 2026-11-14 evidence list; found in this review. |
| `BuyItem` | 1527-1607 | Deducts the price (`:1584-1586`) **before** adding, and ignores `add_to_slots`' result (`:1597`): full inventory = gold gone, no item, and `item.bought` still fires. Stock never decremented (see above). |
| `TakeAllFromContainer` | 1659-1715 | Overflow destroyed. **Fixed in the uncommitted `feature/take_all_overflow` worktree** (takes what fits, leaves the rest, `inventory.full:player`, `container.looted` only when empty). |
| `Open*/Close*/ToggleInventory` | 1335-1365, 1519-1525, 1609-1657 | `OpenInventory` always +1 (no already-open check); `CloseInventory`/`CloseShop`/`CloseContainer` always -1 even when that panel is not open. The demo's `entity.exited:merchant_01 -> CloseShop` (`3rd_person_game_demo/logic/state_machine.ron:174`) fires on every walk-past, so walking past the merchant with the inventory open drops the counter to 0 while the inventory is still visible. `OpenShop`/`OpenContainer` are already guarded (`:1400`, `:1643`); `Despawn` closes the container correctly (`:321-337`). |
| `entity_spawner.rs:75-91` | initial items | Warns on overflow; fine. |

### UI wiring facts that constrain the design

- Every shop row's Buy button emits `ui.button_pressed:buy_item:{item_key}` (`action_executor.rs:1506`), and the
  FSM matches events by exact string (`fsm_interpreter.rs:59, 75`), so every project needs one hand-written rule
  per stocked item (`state_machine.ron:176-179`). Any new per-item verb (use, sell) would multiply this.
- Inventory slots carry no `Button`, so no `Interaction` (`scene_loader.rs:2612-2620`). Bevy 0.18's
  `ui_focus_system` only sets `Pressed` for **left** mouse or touch (`bevy_ui-0.18.0/src/focus.rs:176, 188`):
  right-click-to-use needs its own system (hovered slot + `ButtonInput<MouseButton>::just_pressed(Right)`), and
  touch has no right-click at all.
- No drag-and-drop exists in core (no `Pointer<Drag*>`/`Pickable` use). Bevy's `ui_picking` feature is part of
  bevy's default feature groups; whether it is actually compiled into this build was **not verified**.
- `ActionQueue` is FIFO and the executor drains it with `pop_front` in one loop (`runtime/actions.rs`,
  `action_executor.rs:39`), so actions the executor pushes run the same frame but **after** everything already
  queued (the existing `BuyItem -> ShowFloatingText` already relies on this).

---

## 2. Proposed target model

### Ownership and lifecycle rules

1. **An item exists in exactly one place: an inventory slot, or (currency) a stat.** Every move goes through
   the transaction primitive; no action calls `add_to_slots`/`remove_from_slots` directly any more.
2. **Inventories:** `PlayerInventory` (global, persists across scenes) and `Inventory` (per entity, dies with
   it). Addressed by an internal `InvRef { Player, Entity(Entity) }`, resolved once from the RON string
   (`"player"` or a spawn id). `InvRef` is the forward-compat hook for per-player inventories later
   (`Player(index)`); do not build that now.
3. **`PlayerInventory.max_slots` comes from data, not a hardcoded 20.** Recommended source: a new optional
   `ProjectConfig`/player-prefab field (see open question 6); fallback stays 20 so nothing changes for existing
   projects. Scene load warns when the `InventoryPanel` shows fewer slots than `max_slots`.
4. **Counts live only in slots.** A "total count of item X" is computed (`count_item(slots, key)`), never stored
   (no second counter to drift; ~20-40 slots, trivially cheap).
5. **Currency is a stat, not a slot item.** Any transaction that gives a `currency_stat` item to the player
   credits the stat instead, in every action (not just TakeAll). Currency stat resolution uses one shared rule,
   the action bar's `resolve_cost_source` order (player `StatMap` first, then `LoadedStats`), so `BuyItem` and
   bar costs can never disagree about whose gold it is.
6. **Merchant stock is runtime state on the merchant entity:** a `MerchantStock` component inserted at spawn from
   `MerchantDef` (the def stays the immutable template). `BuyItem`/`OpenShop` read the component, never the
   catalog. It resets when the merchant respawns, i.e. on scene load, which makes the existing doc claim true by
   construction. Cross-scene persistent stock is out of scope (open question 3).
7. **Bar slots hold references, never copies** (WoW model): a slot names an item *type*; the count shown is the
   live total across stacks; at 0 the slot greys out (same dim path as an unaffordable stat cost); clearing the
   slot removes only the reference.

### The item transaction primitive

A pure function in `capabilities/inventory.rs` (or a new `capabilities/item_txn.rs`), no ECS types inside, so it
is unit-testable without an `App`:

```rust
pub enum TxnOp {
    Take { from: InvKey, item: String, count: u32 },   // must remove exactly `count`
    Give { to: InvKey, item: String, count: u32 },     // must fit exactly `count` (currency items credit the stat)
    Stat { key: String, delta: f32 },                  // negative delta must not go below 0
    Stock { item: String, delta: i32 },                // merchant stock, must not go below 0 (None = unlimited)
}
pub enum TxnError { NotEnoughItems, NoRoom, InsufficientFunds, SoldOut, Unsellable, Undestroyable, UnknownItem }
pub fn plan(ops: &[TxnOp], view: TxnView) -> Result<TxnPlan, TxnError>; // works on clones of the touched slot Vecs/values
pub fn commit(plan: TxnPlan, view: TxnViewMut);                       // writes the clones back
```

- **All-or-nothing by clone-and-swap:** apply every op to clones of the affected slot vectors / stat values /
  stock entries; if any op fails, drop the clones. Slot vectors are tiny, so this is cheaper than computing
  "room" separately, and it correctly handles a transaction that both frees and fills a slot (sell, swap).
- **Ops apply in order on a `Vec`** (no map iteration), so the result is deterministic; container and stock
  types are `Vec`/`BTreeMap` only. The function runs inside `action_executor_system` (Update), like every item
  action today; nothing here needs `FixedUpdate` (no physics, no per-tick sim state).
- **Defined failure event per verb, success events unchanged:** keep `inventory.added:*`, `inventory.removed:*`,
  `inventory.full:{inv}`, `item.bought:{item}`, `shop.insufficient_funds`, `container.looted:{id}` exactly as
  they are (designer contract), and add `shop.sold_out:{item}`, `item.sold:{item}`, `item.sell_refused:{item}`,
  `item.used:{item}`, `item.use_failed:{item}`, `item.destroyed:{item}`, `inventory.transfer_failed:{from}:{to}:{item}`.
  A failure never emits a success event (today's `BuyItem`/`TransferItem` do).
- **Who uses it:** `BuyItem` = `[Stat(-price), Stock(-1), Give(player)]`; `SellItem` = `[Take(player), Stat(+price)]`;
  `TransferItem` = `[Take(from), Give(to)]` atomic (the full count or nothing, failure event); `UseItem` =
  `[Take(player, consume)]` then the use actions; `DestroyItem` = `[Take(player)]`; `RemoveItem` = `Take`.
  **Partial-by-design actions stay partial**, built as a loop of single-stack transactions: `AddItem` (adds what
  fits + `inventory.full`, today's contract) and `TakeAllFromContainer` (per stack, the `take_all_overflow` rules).
  Whether `TransferItem` becomes strict or partial is open question 7.

### Use, sell, destroy

- `ItemDef.use: Option<ItemUseDef>` with `actions: Vec<Action>` and `consume: u32` (default 1). `Action::UseItem(item)`
  runs the transaction; on success the executor pushes the item's actions with `{self}` rewritten to the player
  spawn id (`rewrite_self`, `action_substitution.rs`) and emits `item.used:{item}`; on failure emits
  `item.use_failed:{item}` and pushes nothing. This is the only way to make "potion heals only if a potion was
  actually consumed" atomic: a bar slot with `do_actions: [RemoveItem(..), ModifyStat(..)]` (possible today) heals
  even when `RemoveItem` removed nothing. The executor is already a sanctioned `ActionQueue` pusher.
- Sell price is an **item** property (`ItemDef.sell_price: Option<u32>`, `None` = unsellable, which also covers
  quest/key items). `ShopEntry.sell_price` becomes an optional per-merchant override (see schema impact).
  Selling requires an open shop and uses the merchant's `currency_stat`.
- `ItemDef.destroyable: bool` (default `true`); key/quest items set `false`. Destroy always asks for confirmation
  in the panel UI; the `DestroyItem` action itself does not (designer-fired actions must not block on UI).

### Engine-owned item verbs and the per-item wiring problem

Panel buttons (Buy, and new Use/Sell/Destroy on inventory slots) should keep emitting a `ui.button_pressed:*`
event **and** run the default action when no FSM rule matched that exact event, using the same mechanism as the
action-bar intent layer (`HandledIntentSlots`: the interpreter records a matched event, a flush system skips the
default). Result: existing projects with hand-written `buy_item:*` rules keep working unchanged (their rule wins,
no double-buy), new projects need zero per-item rules, and a designer can still override any one item. This needs
a generalisation of `HandledIntentSlots` from slot keys to "default-backed events" (open question 2).

### Panel open-state

Replace `panels_open: u8` with an idempotent `OpenPanels(BTreeSet<PanelKind>)` (`Inventory`, `Shop`, `Container`),
the same lesson as `SimClock`'s hold set: Open inserts, Close removes, a Close of a panel that is not open is a
no-op. `LoadScene` clears it. The three gates read one helper, `world_input_blocked()`, which returns true when any
open panel's kind is "modal". Whether the inventory panel blocks world interaction at all is open question 1 (it
is what made the gate unusable during the playtest, and that is today's intended behaviour, not only the drift).
Add the `debug!` line at each gate that backlog line 70 already asks for, in the same slice.

---

## 3. RON surface

Everything below is **additive**; no `schema_version` bump. Two fields change type in a backward-compatible way
(flagged). `items.ron` would become an Action-bearing file for the first time: its loader must surface a parse
failure visibly (see the schema-tightening note: several Action-bearing loaders have no `Failed` arm), and
`ironhold validate` must walk `ItemDef.use.actions` like FSM actions.

### items.ron

```ron
"health_potion": (
    display_name: "Health Potion",
    icon_index: 2,
    max_stack: 10,
    sell_price: 5,                    // NEW Option<u32>; omitted = unsellable
    use: (                            // NEW Option<ItemUseDef>
        actions: [ ModifyStat(key: "{self}.health", delta: 50.0), PlaySound(key: "drink") ],
        consume: 1,                   // default 1
    ),
),
"old_key": (
    display_name: "Old Key",
    stackable: false,
    destroyable: false,               // NEW, default true; no sell_price -> unsellable
),
```

(`ModifyStat`'s `{self}.health` key form is an assumption about the per-player stat-pool key format; verify
against `docs/20_data_formats.md` before writing the example into docs.)

### Actions (new variants, `schema/actions.rs`)

```ron
UseItem("health_potion")                       // player inventory; item.used / item.use_failed
SellItem("iron_sword")                         // needs an open shop; item.sold / item.sell_refused
DestroyItem(item_key: "iron_sword", count: 1)  // item.destroyed; refused for destroyable: false
```

`BuyItem`, `AddItem`, `RemoveItem`, `TransferItem`, `TakeAllFromContainer` keep their RON shape; only semantics
tighten (atomic, failure events). New variants need: an executor arm, `ironhold query actions`, `validate`
item-key reference checks, and (if any field ever holds an entity id) the four `{self}`/`{target}` sites listed
in `schema/CLAUDE.md`.

### Merchant (prefabs.ron), unchanged shape

```ron
merchant: (
    currency_stat: "gold",
    stock: [
        (item_key: "health_potion", buy_price: 10),                 // sell_price now optional (override)
        (item_key: "old_key",       buy_price: 150, stock_count: 1), // now really sells once per scene load
    ],
),
```

**Type change 1:** `ShopEntry.sell_price: u32 -> Option<u32>`. Every authored `sell_price: 5` still parses because
the RON loader enables `implicit_some` (`schema/ron_loader.rs:8, 73`). Not a break.

### Events (designer reactions, `state_machine.ron`)

```ron
( event: "inventory.full:player",       do_actions: [ ShowFloatingText(entity: "player_01", text: "Bags are full") ] ),
( event: "shop.sold_out:old_key",       do_actions: [ PlaySound(key: "click") ] ),
( event: "item.use_failed:health_potion", do_actions: [ ShowFloatingText(entity: "player_01", text: "No potions left") ] ),
```

### Action-bar item slot (scene.ron)

```ron
ActionBar((
    id: "main_bar",
    slots: [
        ( key: "1", do_actions: [ PlayAnimationOn(target: "player_01", clip: "attack") ], cooldown_secs: Some(1.0) ),
        ( key: "4", item: "health_potion", cooldown_secs: Some(2.0) ),   // NEW: item reference, shows count, greys at 0
        ( key: "5", assignable: true ),                                  // NEW: empty slot the player can fill (S6)
    ],
))
```

**Type change 2:** `ActionSlotDef.do_actions` gains `#[serde(default)]` (was required). Every existing slot still
parses. `validate` enforces exactly one of: non-empty `do_actions`, `item`, or `assignable: true`. An `item` slot
fires `UseItem(item)` through the existing intent path (`intent.slot.{key}:{player}`, cooldown, `cost` all still
apply), so a designer can still override it with a rule. Icon defaults to the item's `icon_sheet`/`icon_index`.

### Interactable, unchanged

`requires_item` stays a check. Consuming a key stays explicit (`RemoveItem` in the `entity.interacted:*` rule);
an optional `consume_required_item: bool` is listed under open question 5, not proposed by default.

### WASM / determinism notes

- No new dependencies; no threads; no hash-map iteration on any of these paths (`Vec` slots, `BTreeSet` panels,
  `BTreeMap`/`Vec` stock; `HashMap` stays lookup-only with `// det:` markers per `tests/determinism_lint.rs`).
- All item mutation stays in `action_executor_system` (Update, `GameplaySet::Execute`), UI refresh stays in
  `GameplaySet::PostExecute` (`lib.rs:512-516`). New UI systems (slot hover/right-click, pick/place, bar count
  label) join `EmitSet`/`PostExecute` explicitly; `tests/schedule_order_tests.rs` will flag a missing set.
- Per-frame cost: bar count labels need `count_item` per item slot per frame (tens of slots x ~20 inventory slots);
  guard the `Text` write so change detection fires only on a change (crate rule).
- The shop panel must re-render or update its `[N]` stock label after a purchase; prefer a marker-driven update
  system over despawning and rebuilding rows (WASM UI rebuild cost, and it avoids a frame with no rows).

---

## 4. Recommended slicing

Order is chosen so each slice is small, independently shippable, and the risky shared piece (the primitive)
lands before anything depends on it.

| # | Slice | Size | Depends on | Plan-review first? |
|---|---|---|---|---|
| S0 | Merge `feature/take_all_overflow` as is (it is consistent with the target model). Note its worktree's `pkg/` holds a dev build (156 MB wasm): `git checkout -- pkg` before committing, as usual. | XS | — | No (already reviewed) |
| S1 | `OpenPanels` set replaces the counter; idempotent Open/Close for all three panels; the backlog-70 `debug!` lines; decide inventory modality (Q1). Tests: open twice/close once, close-not-open, walk-past `CloseShop` with inventory open. | S | — | Light (Q1 only) |
| S2 | Transaction primitive (pure fn + unit tests) and move `BuyItem` (funds + room + stock atomic), `TransferItem`, `AddItem` currency routing, `RemoveItem` failure event onto it. `MerchantStock` component; shop stock label refresh; fix the `catalog.rs:1693` comment and the docs row. Shared currency resolution. `PlayerInventory.max_slots` from data (Q6). | M | S0 | **Yes** (primitive API, event names, Q4, Q6, Q7) |
| S3 | `ItemDef.use` + `UseItem`; inventory slots become `Button`s; click (or right-click) a slot to use; `item.used`/`item.use_failed`; the "default-backed event" mechanism for panel verbs (Q2), applied to Buy too so per-item buy rules become optional. | M | S2 | **Yes** (Q2 mechanism, `items.ron` as an Action-bearing file, input on touch) |
| S4 | `SellItem` + `DestroyItem`; `sell_price`/`destroyable` on `ItemDef`; `ShopEntry.sell_price` optional; slot context menu (Use / Sell when a shop is open / Destroy) with a confirm step for Destroy. | M | S3 | Yes, with ux-gamedesigner (menu vs modifier-click, confirm UX) |
| S5 | Authored item slots on the action bar: `item:` field, count label, dim at 0, icon from the item. | S | S3 | Light |
| S6 | Player-assignable slots: `assignable: true`, pick from inventory then click a bar slot to place, clear without touching the inventory; assignments persist across scenes (Q8). Click-to-pick first; real drag later on top of the draggable-windows work. | M | S5 | **Yes** (input model, persistence, local co-op) |
| S7 | Vendor buy-back list (`MerchantStock.buyback`, capped, cleared on scene load). | S | S4 | Icebox until asked |

Fold-ins: the existing backlog items "Silent `panels_open` gate logs nothing" (line 70) go into S1; "Hover tooltips"
(line 323) should land after S3 so it targets `Button` inventory slots; "Extract a shared panel-chrome helper"
(line 326) is worth doing at the start of S4, before a context menu adds a fourth hand-copied UI block. Loot v1
(line 137) and Monster drop pickups (line 136) should deliver through S2's primitive, not new add code.

### What I would NOT do

- No generic item instance model (UUIDs, durability, per-instance affixes). Stacks of a type are enough for every
  feature listed; instance data is an equipment-system question.
- No per-player inventories in this batch (`InvRef` keeps the door open; local co-op keeps one shared inventory).
- No implicit key consumption on `requires_item`; no "tags drive behaviour" (`tags: ["consumable"]` stays a
  designer label; behaviour comes from typed fields like `use:`/`sell_price`/`destroyable`).
- No persistent cross-scene merchant stock or save/load of inventories (no save system exists yet).
- No true drag-and-drop in S6 (no drag infrastructure, touch and gamepad need the click model anyway).
- No direct `ActionQueue` pushes from new UI systems other than through the default-backed-event flush (Q2).
- No weight/encumbrance (`weight` stays unused).

---

## 5. Open questions (Frank's decisions, answered 2026-11-14)

**Decisions (Frank, 2026-11-14):** 1 agreed (the inventory panel does not block world interaction; shop and container panels still do). 2 agreed (default-backed events). 4 agreed (player stats first, then the shared pool). 5 agreed (keys are not consumed automatically; `RemoveItem` stays explicit). 6 agreed (project-level inventory size, default 20, load-time warning). 7 agreed (`TransferItem` moves what fits and emits `inventory.full`). 8 agreed (assigned bar slots persist across scenes). 9 agreed (click menu plus right-click shortcut).
**3 was answered "false" (reading: do NOT reset merchant stock on scene load, so it persists across scene loads).** This contradicts the recommendation and the "What I would NOT do" list above; read as an in-memory resource keyed by merchant spawn id that lives for the session, with no save/load. Confirm that reading at the S2 plan-review.
**Added by Frank:** health/mana checks for potions, and item-usage pre-conditions in general, are their own backlog item (see "Item use conditions" in `planning/backlog.md`). They should be general (other potions and poisons, equip conditions, collecting items and merging them into new items), not one-off checks.


1. **Should the inventory panel block world interaction (F-interact, walk-over pickup, Tab targeting)?** Today any
   open panel blocks all three, which is why the gate "did nothing" with bags open. *Recommendation:* inventory
   non-modal (WoW-like); shop and container stay modal (they belong to an interaction).
2. **Per-item panel verbs: default-backed events (rule wins, otherwise the engine runs the default) or keep
   requiring a RON rule per item?** *Recommendation:* default-backed; backward compatible with the demo's existing
   `buy_item:*` rules.
3. **Merchant stock lifetime:** per scene load (component on the merchant) or persistent per merchant id across
   scenes (resource)? *Recommendation:* per scene load now; revisit with a save system.
4. **Currency owner in co-op:** shared gold (`LoadedStats`) or per-player gold when the player's `StatMap` has the
   stat? *Recommendation:* the action-bar rule (player first, global fallback) everywhere, so it is one rule.
5. **Key items:** keep "check only, consume via explicit `RemoveItem`", or add `consume_required_item: true` on
   `interactable`? *Recommendation:* keep explicit for now; add the flag only if a second project needs it.
6. **Where does the player inventory size come from?** `ProjectConfig.player_inventory_slots`, the player prefab, or
   the panel's `columns * rows`? *Recommendation:* a project-level field (the inventory persists across scenes, so a
   per-scene panel is the wrong owner), default 20, plus a load-time warning when the panel shows fewer slots.
7. **`TransferItem` when the destination is full:** all-or-nothing (strict) or move what fits (like TakeAll)?
   *Recommendation:* move what fits and leave the rest in the source, plus `inventory.full:{to}`, for consistency
   with TakeAll; strictness belongs to Buy/Sell.
8. **Assigned bar slots:** persist across scenes, or reset to the authored layout on scene load? *Recommendation:*
   persist (keyed by bar id + slot key), since the inventory persists too.
9. **Using from the inventory:** left-click opens a small Use/Sell/Destroy menu (works with mouse and touch), with
   right-click as a direct Use shortcut on desktop? Or right-click only? *Recommendation:* the menu plus the
   shortcut; right-click alone leaves touch users with no way to use an item.

## Acceptance criteria (for the batch)

- Given a full inventory, when the player buys an item, then no gold is spent, no `item.bought` fires and
  `inventory.full:player` fires.
- Given `stock_count: 1`, when the player buys it, then the shop shows it sold out and a second buy emits
  `shop.sold_out:{item}`, until the next scene load.
- Given the inventory panel open, when the player walks past a merchant (`CloseShop` fires), then the inventory
  state and world-input gating are unchanged.
- Given 3 potions in two stacks and a bar slot `item: "health_potion"`, then the slot shows 3; using it heals and
  shows 2; at 0 the slot is greyed and pressing it emits `item.use_failed:health_potion` without healing.
- Given an item with `destroyable: false` or no `sell_price`, then Destroy/Sell are refused with their failure event.
- Clearing an assigned bar slot never changes the inventory.

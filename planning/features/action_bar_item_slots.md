# Feature: Item slots on the action bar (item system S5)

_Status: In Progress (plan-reviewed 2026-11-15; system-architect and ux-gamedesigner-reviewer findings folded in, Q1-Q4 settled)_
_Planned at: `a40dbd7` (2026-11-15)_

Slice S5 of `planning/features/item_system_review.md`. Builds on S3 (`UseItem`, `planning/features/done/item_use.md`) and the action bar's intent
path (`capabilities/action_bar.rs`, `docs/dev/action-bar-input-routing.md`). NOT part of this slice: player-assignable slots (S6), item
cooldowns on `use:`, hover tooltips, use conditions (backlog), making `{self}` follow the slot's `owner_player` (see Out of scope).

## What
A designer can put an item on an action bar slot: `( key: "6", item: "health_potion", cooldown_secs: 2.0 )`. The slot shows the item's icon, the
total number of that item in the bag (over every stack) in a corner, dims when there are none, and when pressed (key, gamepad button or click) uses the
item exactly like `UseItem("health_potion")`. Today a potion on the bar needs hand-written `do_actions: [ UseItem("health_potion") ]`, an icon picked
by hand, and shows no count; a gamepad or keyboard player has no other way to drink a potion (the bag menu is mouse and touch only).

## Why
It is the only way a keyboard or gamepad player uses an item, and the base S6 (the player puts a consumable on a free slot) builds on: the same slot,
icon, count label, dimming and firing rules, with the item chosen at runtime instead of authored.

## Decisions (plan-review, 2026-11-15)
- **The item's icon is resolved exactly as the bag resolves it**, at runtime: the item's `icon_sheet`, else the `InventoryPanel`'s sheet, always read against
  the panel's grid (`LoadedInventoryUi.icon_atlases` / `panel_icon_sheet`, built by the panel). Not against the bar's sheet and grid: the demo's `skill_bar`
  uses `icons_status_effects`, so a bar-sheet fallback would show cell 2 of the wrong sheet ("Dual Strike") with no warning. No "bar grid must equal bag
  grid" rule exists any more.
- **`icon_index` cannot say "unset"** (a `u32` defaulting to 0), so the slot's own picture is switched on by `icon:` (a non-empty sheet key), not by
  `icon_index`: with `icon:` set the slot keeps today's static icon (`icon` + `icon_index`); without it the item's icon is used. `icon_color`
  (already an `Option`) overrides the tint on its own.
- Q1 yes (a `consume: 0` item shows its count). Q2 refuse with no cooldown and no cost. Q3 as above. Q4 log the co-op `{self}` limitation, and warn at authoring
  time (`--strict`) for an item slot on a bar with `owner_player > 0`.

## Approach

### 1. Schema (additive, no `schema_version` bump)
- `ActionSlotDef.item: Option<String>` (an `items.ron` key). `do_actions` gains `#[serde(default)]`; every existing slot still parses, and `( key: "6" )` is a
  legitimately empty slot (the shape S6 wants). The only readers of `do_actions` are the loader, `action_needs_target`, the rewrite in the input system
  and one CLI loop.
- The item lives on the slot entity as its own component `ActionSlotItem(pub String)`, **not** a field of `ActionSlotUi` (about 37 struct literals in tests
  would change, and S6 must insert and remove it at runtime). Systems read it as `Option<&ActionSlotItem>`.
- `item` together with a non-empty `do_actions` is an `ironhold validate` error (`action_slot_item_and_actions`); at load the item wins and a `warn!`
  names the bar and slot (`slot "6" on bar "skill_bar" has both item: and do_actions: - use one (item: uses the item's use: block)`).

### 2. Firing (`action_bar_input_system`, the existing intent path)
- An item slot's actions are `[UseItem(item)]` (the same `pending` / `flush_pending_intent_system` path as `do_actions`): same keyboard, gamepad and click
  sources, same `cooldown_secs` and `cost` (`UseItem` front-inserts its effects at execute time, so they run before the cost's `ModifyStat`), the same
  `intent.slot.{key}:{player}` event (a rule on it replaces the slot), `action_bar.pressed:{key}` and `action_bar.activated:{key}`. `UseItem` has no `{target}`.
- **The no-item gate** sits after the owning player is resolved and after the gamepad cooldown check, **before the cost check**: cooldown, then no-item, then
  cost. The bag holds none of the item (`item_txn::count_item` over the shared `PlayerInventory`) -> emit `action_bar.no_item:{key}` and `continue`: nothing is
  pushed to `pending`, **no cooldown starts, no cost is spent, and neither `action_bar.pressed` nor `intent.slot.*` is emitted**, so a rule on the intent never
  sees a press on an empty slot (it mirrors `insufficient_resource`).
- The gate reads the bag as the last executor left it; two slots of one item (two bars) with one unit held both pass the gate in the same frame, both start a
  cooldown and one `UseItem` emits `item.use_failed`: accepted, documented.
- In local co-op `{self}` in the item's `use:` is still the primary player (S3), so a slot on player 2's bar heals player 1. Documented in "Item slots" **and**
  in docs/20's "Per-player action bars" section; backlog item; `--strict` warns for an item slot on a bar with `owner_player > 0`.

### 3. Display
- **Loader:** an item slot always gets an `Icon` child with a `SlotIcon` marker (hidden when nothing resolves) and a count label child `SlotItemCount`
  (bottom-left; the key hint stays bottom-right). A slot with its own `icon:` keeps today's static icon child and gets no `SlotIcon`.
- **One refresh system in `GameplaySet::PostExecute`** (after the executor, like `inventory_ui_system`, so the label never lags a use), gated on
  `Added<ActionSlotItem>` / `Changed<ActionSlotItem>` or `PlayerInventory.is_changed()` or the item catalog changing: for each item slot it writes the icon
  (image, atlas index, tint from the item via a pure `resolve_item_icon(catalog, ui, item_key)` shared with `inventory_ui_system`, no second copy of the
  resolution) and the count, only when they differ. Idle frames cost nothing. S6 will only insert/remove `ActionSlotItem`.
- The count label: the font size follows `slot_size` (about 0.3 x, never below 10), text `{n}` capped at `99+`, grey at 0 (not hidden: the player sees "0").
- **Dim:** `action_bar_visual_system` reads the count stored on `SlotItemCount` (a `u32`, written by the refresh system) and uses the same 0.45 alpha as
  "blocked by cost" at 0; a running cooldown still wins. A one-frame lag on the dim is harmless.
- With no `InventoryPanel` in the scene the item's icon cannot resolve (the atlases come from the panel): such a slot needs its own `icon:`; load-time `warn!`
  and a `--strict` warning say so. Count and dim still work.
- No item catalog (`items.ron` failed to load or none): the slot has no icon, reads 0, refuses; a scene-load `warn!` names the slot. An unknown item key or an
  item with no `use:` also gets a scene-load `warn!` (the `validate` errors are the real net).

### 4. CLI (`ironhold validate`)
- In the loop that adds each slot's `do_actions` to the collected actions, push a synthesized `Action::UseItem(item)` for an item slot: the existing unknown-item
  (`missing_reference`) and `use_item_without_use_block` checks then cover item slots with no new error kind. No separate `action_slot_item_without_use`.
- New error `action_slot_item_and_actions`. `--strict`: a currency item on a slot; an item slot with no `icon:` in a project with no `InventoryPanel`; an item slot
  on a bar with `owner_player > 0`. `ironhold query` never lists slots: no change. No check for `action_bar.no_item` rules (no `action_bar.*` event is known to the
  CLI today; deferred).

### 5. Demo (`3rd_person_game_demo`)
Two item slots on the bar (health and mana potion) with `gamepad_key: "DPadUp"` / `"DPadDown"`, which needs `inputs.gamepad_index: 0` on the player prefab (a
`gamepad_key` without it is a hard `validate` error), plus a rule on `action_bar.no_item:6` showing "No potions left". RON in the demo's style (no `Some(...)`).
Check the bar still fits (it ends near x=884 today).

### Events
New: `action_bar.no_item:{key}` (an item slot was pressed with none in the bag; no cooldown; `pressed`, `activated` and `intent.slot.*` do not fire). Existing
`on_cooldown`, `insufficient_resource`, `pressed`, `activated` apply unchanged; using the item still emits `item.used:{item}` / `item.use_failed`.

### RON authoring example
```ron
ActionBar((
    id: "main_bar",
    slots: [
        ( key: "1", do_actions: [ PlayAnimationOn(target: "player_01", clip: "attack") ], cooldown_secs: 1.0 ),
        ( key: "6", item: "health_potion", cooldown_secs: 2.0, gamepad_key: "DPadUp" ),   // icon, count and dim come from the item (and the bag's sheet)
        ( key: "7", item: "mana_potion", icon_color: (0.5, 0.5, 1.0, 1.0) ),               // an own icon_color overrides the tint
        ( key: "8", item: "war_horn", icon: "icons_status_effects", icon_index: 5 ),         // an own icon: switches the whole picture to this sheet
    ],
))
// state_machine.ron: optional reaction
( event: "action_bar.no_item:6", do_actions: [ ShowFloatingText(entity: "player_01", text: "No potions left") ] ),
```

## Tasks
- [ ] Failing tests first: an item slot press queues `UseItem` once through the intent event (and a rule on `intent.slot.6:*` replaces it); none held ->
      `action_bar.no_item:6`, no cooldown, no cost, no `intent.slot`, no `pressed`, no `UseItem`; held -> uses, cooldown starts; the gate sits after the cooldown
      check and before the cost check; the count label shows the total over stacks, follows the bag the same frame and caps at `99+`; the slot dims at 0 and
      not at 1; cooldown beats the empty dim; a plain slot has no count label or `SlotIcon`; the icon comes from the item's sheet and the panel's grid (not the
      bar's), an own `icon:` keeps the static icon and `icon_color` overrides the tint; item with `do_actions` warns and the item wins; no catalog: no icon,
      reads 0; paused clock and inspector gates unchanged
- [ ] Schema (`ActionSlotDef.item`, `do_actions` default), `ActionSlotItem`, loader (icon child with `SlotIcon`, count label, the load-time `warn!`s)
- [ ] `capabilities/inventory.rs`: extract the pure `resolve_item_icon` and use it from both `inventory_ui_system` and the new refresh system
- [ ] `action_bar_input_system` no-item gate (+ `PlayerInventory` param), the refresh system in `PostExecute`, `action_bar_visual_system` dim; `schedule_order_tests`
- [ ] `ironhold validate`: the synthesized `UseItem`, `action_slot_item_and_actions`, the three `--strict` warnings, `validate_cross_file` tests both directions
- [ ] Demo (items on the bar, gamepad, rule), docs: `docs/20` (`ActionSlotDef` `item` row; rewrite the "Using items" gamepad paragraph to recommend `item:` and
      say the old `do_actions: [UseItem]` form still burns the cooldown; "Item slots" paragraph with the icon rule, the refusal events and the co-op note;
      "Per-player action bars" co-op note), `docs/30` (events, which fire on a refusal), `docs/15` validate rows, `STATUS.md`, `capabilities/CLAUDE.md`,
      `docs/dev/action-bar-input-routing.md`, tests table
- [ ] Reviews, full core test loop, `cargo check -p ironhold_cli`, `cargo test -p ironhold_cli`, `ron_lint`/`ron_validation`

## Out of scope
Assigning items at runtime and persistence (S6: an assignments resource the loader consults, since `LoadScene` rebuilds the bar), item cooldowns on `use:` (an
inventory click still bypasses a slot's `cooldown_secs`), `{self}` following the slot's `owner_player` in co-op (UseItem acts on the primary player), hover
tooltips and a slot name (`label` is still not rendered), use conditions (a potion at full health is still consumed), a `consume: 0` item's count meaning "uses
left", an unreachable-`no_item`-rule check in `validate`.

## Open questions
None blocking.

## Acceptance criteria
- Given `( key: "6", item: "health_potion" )` and 3 potions over two stacks, then the slot shows the potion icon (from the bag's sheet) and `3`; pressing 6 (or
  clicking it, or the gamepad button) uses one: `item.used:health_potion` fires and the label reads `2` in the same frame.
- Given none held, then the slot is dimmed with a grey `0`; pressing it emits `action_bar.no_item:6`, uses nothing, spends no cost, starts no cooldown and emits
  no `intent.slot.6:*` or `action_bar.pressed:6`.
- Given a rule on `intent.slot.6:{player}` then only the rule runs when the bag holds the item (as for every slot).
- Given `icon: "icons_status_effects"` on an item slot, the slot shows that sheet's `icon_index`, not the item's icon.
- `ironhold validate` rejects an item slot that also has `do_actions`, an unknown item and an item with no `use:`.

## Implementation notes (2026-11-15, from the step-4 reviews)
- **The gate refuses when fewer are held than one use takes** (`item_needed` = `use.consume`, at least 1), not only at 0: an elixir that consumes 2 with one in the
  bag would otherwise spend the cost and start the cooldown and then fail inside `UseItem`. The count turns grey and the slot dims below that threshold.
- **`ActionSlotItem` is the one source of what the slot does.** The input system fires `UseItem(item)` from it; the loader bakes nothing into `do_actions` for an item
  slot (the plan said `do_actions` would be `[UseItem]`; two copies could drift, and S6 inserting the component would not have worked). Removing the component
  clears the label and hides the icon. The icon and count children are still spawned only by the loader: **S6 must spawn them for an assignable slot.**
- The refresh system is ordered after `slot_menu_sync_system`. The no-icon warning repeats if an item's icon stops resolving after having resolved; it is silent
  again for the rest of the run otherwise (`validate --strict` is the real check, which now counts a panel in any scene of the project).
- Not changed: `ironhold validate` is silent for an item slot when the project has no `items.ron` (the same gap as every `UseItem`); an `icon_index` beyond the
  panel's grid shows the whole sheet (also true in the bag); the `validate` messages for an unknown item or one with no `use:` name `UseItem(...)` (the slot's
  `item:`), documented in `docs/15`.

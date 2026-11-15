# Feature: Player-assignable action bar slots (item system S6)

_Status: Ready (plan-reviewed 2026-11-15; system-architect and ux-gamedesigner-reviewer findings folded in, Q-A..Q-F settled)_
_Planned at: `8c6e254` (2026-11-15)_

Slice S6 of `planning/features/item_system_review.md`. Builds on S5 (item slots, `planning/features/done/action_bar_item_slots.md`) and S4 (the bag menu,
`planning/features/done/item_sell_destroy.md`). Frank's earlier decisions (2026-11-14) that apply: **assigned slots persist across scenes** (Q8), **a click
menu plus right-click** is the bag's input model (Q9), click-to-pick first and real drag later. NOT part of this slice: drag and drop, gamepad or keyboard
assigning, saving assignments across page reloads (no save system exists), a one-time "you can fill the bar" hint, item cooldowns.

## What
A designer marks bar slots `assignable: true`. The player opens the bag, picks a consumable and puts it on one of those slots, moves it between slots and
takes it off again, the way an RPG hotbar works: the slot then behaves exactly like an S5 item slot (icon, bag count, dims at 0, key/gamepad/click uses it).
What the player assigned is kept when the scene changes. The same operations are two actions a designer can fire (`AssignBarSlot`, `ClearBarSlot`), for example
a tutorial that puts the first potion on slot 6.

## Why
S5 lets a designer decide the layout; most RPGs let the player decide. It is also the only way a keyboard player gets a potion they just found onto a key
without the designer predicting which potion that will be.

## Decisions (plan-review, 2026-11-15)
- **No right-click Clear.** A right click means "Use" in the bag and, in every RPG, "use" on a bar slot; the demo also orbits the camera on the right button with
  no UI-hover guard, so a right-drag over the bar must never empty a slot. Instead **an open bag is edit mode for assignable slots** (see 4): a click on a filled
  assignable slot picks its item up (move or remove) instead of using it; keys, gamepad and a click with the bag closed still use it. (The architect would also
  have allowed right click; Frank may overrule.)
- **Assign mode** (not a menu listing the slots). It lives in `LoadedInventoryUi` (reset whole on a Replace load, so `LoadScene` cancels it for free), the
  hint and its buttons are a strip inside the bag panel, and closing the bag cancels it. The same item may sit on several slots of a bar. `item:` together with
  `assignable: true` is a replaceable default. Assignments are lost on a page reload and kept across scenes (docs say so plainly).
- **One owner and no race.** `inventory_slot_click_system` owns every bar-slot click while the mode is on or the bag is open on an assignable slot; it only
  *writes* a handoff in Emit and the mode is **ended in Execute** (by `default_panel_actions_system`, even when a rule owns the event), never during Emit.
  `action_bar_input_system` is ordered `.after(inventory_slot_click_system)` and folds "mode active" into `click_fired` (before the cooldown check), so a click
  in the mode never uses a slot nor emits `on_cooldown`; it keeps running so the `Ref<Interaction>` baseline advances. Key and gamepad presses still fire in the mode.

## Approach

### 1. Schema (additive, no `schema_version` bump)
- `ActionSlotDef.assignable: bool` (default `false`). `validate` errors: `assignable` with `do_actions` (`action_slot_assignable_and_actions`), `assignable` with an
  own `icon:` (it would cover every assigned item; `action_slot_assignable_with_icon`), and two bars in one scene with the same `id` when either has an assignable
  slot (`action_bar_duplicate_id`: the persistence key).
- `Action::AssignBarSlot { bar: String, slot: String, item_key: String }` and `Action::ClearBarSlot { bar: String, slot: String }`: plain ids, no substitution; every
  exhaustive match (`is_presentation_only` returns **false**, `query.rs`, validate's `all_actions`, both `action_substitution` rewrites, `dialogue.rs:331`).

### 2. State, loader, reconcile
- Resource `ActionBarAssignments`, `init_resource`'d at plugin build (exactly one project loads per app lifetime, so that is the only reset: `LoadScene` never
  clears it). Map `BTreeMap<(bar_id, slot_key), Assignment>` with `enum Assignment { Assigned(String), Cleared }` (a cleared authored default stays cleared), the map
  private behind accessors (a save system can serialise it later).
- The loader gives every assignable slot `AssignableSlot { bar, key, default }` (the authored `item:` kept as `default`), the S5 icon and count children
  **always** (spawn them when `item.is_some() || assignable`; the bar's own sheet icon is skipped for an assignable slot like for an item slot), and an
  `ActionSlotItem` from the resource if there is an entry, else the default. The loader reads the resource through `SceneV2Params` (`spawn_scene_v2` is at the
  16-param limit).
- `apply_bar_assignments_system` (`PostExecute`, gated on the resource changing, `.before(action_bar_item_slot_system)`): a **reconcile**, not a diff: for every
  `AssignableSlot`, desired = the map entry if present, else `default`; insert/replace/remove `ActionSlotItem` to match. An empty assignable slot shows a faint
  ASCII `+` (the count label's text when the slot has no item). In `schedule_order_tests`' manifest.
- **An empty assignable slot refuses:** pressing an assignable slot with no item emits `action_bar.empty_slot:{key}` and `continue`s before the intent is built (today an
  empty slot would still start its cooldown and emit `activated`). Test it.

### 3. Actions and events (executor)
- `AssignBarSlot` writes `Assigned(item_key)` and emits `action_bar.slot_assigned:{key}` and `action_bar.slot_assigned:{key}:{item_key}`; `ClearBarSlot` writes
  `Cleared` and emits `action_bar.slot_cleared:{key}`. A refusal changes nothing and emits `action_bar.assign_failed:{key}`: no live slot with that bar and key, a slot
  that is not `assignable`, an item not in the catalog, with no `use:` block, or a currency item. **The executor checks the live slots** (this scene and overlays,
  through `AssignableSlot`): a slot of a later scene cannot be pre-assigned (documented). `{key}` is the slot key (unique scene-wide); the bar id comes from the slot.
- The action does not require the item to be in the bag (a tutorial may pre-assign); the bag UI only offers items it holds.

### 4. The player's gestures (mouse and touch; default-backed events like S3/S4)
- **Put on bar.** The S4 bag menu gets a row **Put on bar**, shown when the item has a `use:` block and the **current scene has a visible assignable slot**
  (and **Remove from bar (N slots)** when the item is on N slots: it emits one `clear_slot:{key}` per slot). Choosing Put on bar closes the menu and enters assign mode with
  that item; choosing it again cancels. `menu_view` takes "on N slots" and "an assignable slot exists" as plain inputs (unit-testable).
- **The strip.** Inside the bag panel a strip appears while the mode is on: "Choose a highlighted slot for Health Potion" with **[Cancel]** (and **[Remove from bar]**
  when an item was picked up from a slot). Every assignable slot of every bar gets a highlighted border (a colour distinct from the cooldown and no-item dims); the
  hint text and the borders are written only when the mode changes.
- **Assign.** A click on an assignable slot emits `ui.button_pressed:assign_slot:{key}` and writes the handoff `(bar, key, item, from)`; `default_panel_actions_system`
  pushes `AssignBarSlot` (and `ClearBarSlot` for the origin when moving) and ends the mode in Execute. A rule bound on `assign_slot:{key}` replaces it (it cannot see the item).
- **Pick up / move / remove.** With the bag open and no mode, a click on a **filled** assignable slot enters the mode with that slot's item (`from` = that slot): a click on
  another assignable slot moves it, **[Remove from bar]** clears it (emits `clear_slot:{key}`), the same slot or Cancel puts it back. This also clears a slot whose item
  left the bag (a grey 0): the picked-up item is the slot's own, so the "item left the bag" cancel below does not apply to it. With the bag closed a click uses the slot.
- **Cancel (derived from state each frame, in `PostExecute`, not a list of actions):** the bag panel hidden, `!sim.is_running()`, no assignable slot left in the scene, a
  Put-on-bar item that left the bag, `LoadScene` (the resource reset), [Cancel], choosing the row again, or a right click anywhere (mouse). A right click in the mode only
  cancels (it never clears the slot under the cursor); a right-drag to orbit therefore also cancels (accepted). A world click is NOT a cancel; a hotkey uses its slot and the mode stays.
- Not for keyboard or gamepad (they use authored item slots and `AssignBarSlot` from a rule); paused, inspector and no mouse: the gestures do nothing.

### 5. CLI (`ironhold validate`)
Errors: the three in 1; `AssignBarSlot`/`ClearBarSlot` naming a bar and slot that no scene has as an assignable slot (`assign_unknown_slot`, a project-wide index over
`walk_ui_nodes`), an unknown item, an item with no `use:` (through the synthesized-`UseItem` path S5 uses), a currency item. `collect_reachable_ui_triggers` gets an `ActionBar` arm
that inserts `assign_slot:{key}` and `clear_slot:{key}` for every assignable slot. `--strict`: an assignable slot in a project with no `InventoryPanel`; an assignable slot on a
`owner_player` above 0 bar (the S5 `{self}` limit); the S5 item-slot warnings still apply to the authored default.

### 6. Demo (`3rd_person_game_demo`)
Slot 6 stays the fixed health potion; slot 7 becomes `( key: "7", item: "mana_potion", assignable: true )` (a visible replaceable default); a new empty `( key: "8", assignable: true )`.
Rules: a floating text on `action_bar.slot_assigned:8`. The HUD hint becomes "I: bag - click an item for Use / Sell / Destroy / Put on bar". The bar grows to 9 slots.

### Events and RON authoring example
New: `action_bar.slot_assigned:{key}` and `:{key}:{item_key}`, `action_bar.slot_cleared:{key}`, `action_bar.assign_failed:{key}`, `action_bar.empty_slot:{key}`; default-backed clicks
`assign_slot:{key}`, `clear_slot:{key}` (these carry only the slot key; the click events elsewhere carry item keys; documented).
```ron
ActionBar((
    id: "main_bar",
    slots: [
        ( key: "6", item: "health_potion", cooldown_secs: 2.0 ),     // S5: fixed
        ( key: "7", item: "mana_potion", assignable: true ),          // a default the player can replace
        ( key: "8", assignable: true, cooldown_secs: 2.0 ),           // starts empty: a faint +
    ],
))
( event: "scene.ready:town", do_actions: [ AssignBarSlot(bar: "main_bar", slot: "8", item_key: "health_potion") ] ),   // a tutorial; AssignBarSlot with the default puts it back
( event: "action_bar.slot_assigned:8", do_actions: [ ShowFloatingText(entity: "player_01", text: "Slot set") ] ),
```

## Tasks
- [ ] Failing tests first: the actions write the resource and the reconcile inserts/removes `ActionSlotItem`; refusals (unknown slot, not assignable, no `use:`, currency) change
      nothing; a cleared authored default stays cleared after a scene load; assignments survive `LoadScene` and rebuild the bar; an empty assignable slot refuses (event, no cooldown);
      assign mode via the menu row: a click on an assignable slot assigns (the slot is not used in that frame, no `on_cooldown`), the mode ends in Execute even when a rule owns the
      event; pick-up/move/remove with the bag open; a click with the bag closed uses the slot; the cancels (right click, closing the bag, pause, item left the bag for Put on bar
      but not for pick-up, `LoadScene`); keys fire in the mode; per-bar independence; the gates; a rule on `assign_slot:` replaces the built-in; `menu_view` rows
- [ ] Schema (`assignable`, the two actions, every exhaustive match), loader (`AssignableSlot`, children, the resource), reconcile system, empty-slot refusal, the schedule edge
- [ ] Executor arms, the default pusher prefixes (`DEFAULT_BACKED_PREFIXES`), the handoff and end-of-mode in Execute, the bag-menu rows, the strip, the highlight, the derived cancels
- [ ] `ironhold validate` + `validate_cross_file` tests both directions (incl. reachability and the duplicate bar id)
- [ ] Demo; docs: `docs/20` (Assignable slots; the S4 menu table gets the two rows; flip "assigning at runtime is not built yet" and add Put on bar to the not-supported line; a persistence callout:
      kept across scenes only when the bar `id` and slot `key` match in every scene, lost on page reload, a cleared default stays cleared in every scene using that id and key; keyboard/gamepad
      players cannot assign, what they get instead; the co-op note next to assignable slots; the two-form `slot_assigned` event and why the click events carry only the key), `docs/30`,
      `docs/15`, `STATUS.md`, `capabilities/CLAUDE.md`, `docs/dev/action-bar-input-routing.md`, the tests table
- [ ] Reviews, full core test loop, `cargo check -p ironhold_cli`, `cargo test -p ironhold_cli`, `ron_lint`/`ron_validation`

## Out of scope
Drag and drop (backlog "real drag later"), gamepad/keyboard assigning (a bag navigation item; a later "assign to the first free slot" option), saving assignments (no save system), per-player bags, assigning
non-item things (skills), a one-time "the bar can be filled" hint (needs a persisted flag: backlog), an item cooldown shared by every slot of an item, pre-assigning a slot of a scene that is not loaded.

## Open questions
None blocking.

## Acceptance criteria
- Given `( key: "8", assignable: true )` and a potion in the bag, when the player picks Put on bar in the bag menu and clicks slot 8, then the slot shows the potion's icon and count and
  pressing 8 uses it; the click itself did not use anything; after `LoadScene` the slot still holds the potion.
- Given a filled assignable slot and the bag open, when the player clicks it and chooses Remove from bar, then it is empty (a `+`) and a cleared authored default stays empty after a scene load.
- Given assign mode, when the player closes the bag or right-clicks, then nothing is assigned and the mode ends; a click on the world does not end it; a hotkey still uses its slot.
- An empty assignable slot pressed emits `action_bar.empty_slot:{key}` and starts no cooldown.
- `AssignBarSlot` for a non-assignable slot or an item with no `use:` is refused with `action_bar.assign_failed:{key}` and `ironhold validate` rejects it.

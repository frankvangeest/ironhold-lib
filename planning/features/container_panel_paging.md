# Feature: Container panel paging and taking a stack (item system follow-up)

_Status: Ready (plan-reviewed 2026-11-16; system-architect and ux-gamedesigner-reviewer findings folded in, Q-A..Q-E settled)_
_Planned at: `7bddcbd` (2026-11-16)_

Found at the S4 play-test (Frank, 2026-11-15) and logged as "Container panel hides slots it has no room for" in `planning/backlog.md`. A `ContainerPanel` draws only its
first `columns * rows` cells, so a chest with more slots than the panel (the demo's `chest_swords`: 20 swords, a 3x3 panel) hides the rest: they can only be moved
with Take All, and nothing warns the designer. NOT part of this slice: gamepad or keyboard navigation of the panel (the cell-to-slot mapping stays a pure function so it can be added),
dragging items out of a container, a container UI that resizes itself, per-player containers, a search or sort, a count picker.

## What
A container panel with fewer cells than the container has slots gets **pages**: a footer `<  1/3  >` and the mouse wheel over the panel flip between pages of
`columns * rows` cells, so every slot can be seen. A click or tap on a filled slot **takes that stack** into the bag (what fits; the rest stays), so the player no longer
has to take everything. Take All still moves every slot (every page), and now also works with no rule written for it, like Buy. A designer sees no change to a container that fits
its panel, except that cells past the last slot are drawn faded.

## Why
Hidden loot is a real UX bug (the demo's own test aid, `chest_swords`, shows it). Once every slot can be seen, "I can see it but only Take All works" is the next complaint.

## Decisions (plan-review, 2026-11-16)
- **Pages, not a scrolling grid** (touch-friendly, no clipping or scrollbar code, deterministic). The arrows are how players find paging; the wheel is a bonus.
- **The page count follows the container's capacity** (`Inventory.slots.len()`, which never shrinks), not its filled stacks: the layout stays fixed while the player takes items, emptied cells stay as
  empty cells, and taking the last items of page 3 keeps the player on page 3. Only cells past the last slot (on the last page) are drawn faded and inert.
- **A click empties exactly the clicked slot**, never "the last stack first" (the demo's chest of 20 swords would otherwise lose a hidden slot 19 when cell 0 is clicked, and the page count could change under the player).
  The handoff carries `(item_key, slot_index, count)`; the action's `slot: Option<u32>` takes from that slot if it still holds the item, else from the last stack (a designer's `TakeFromContainer` with no slot).
- **The panel keeps one height:** the footer row is always reserved (a fixed minimum height); its contents are hidden with `Visibility`, not `Display`, when there is one page. docs/20 can then give one panel height,
  and no second engine writer of `Node.display` is added.
- **The wheel must not also zoom the camera.** Both camera systems read every `MouseWheel` message and never check the UI, so a marker `CapturesWheel` on the container panel root and on each of its buttons makes
  `camera_orbit_system` and `party_camera_follow_system` skip zoom while any marked node has an `Interaction` other than `None`. The same check (any marked node, not the root: the cells, arrows and Take All are `Button`s and
  the focus system stops at the first `Block` node) decides whether the wheel pages. One notch or one trackpad swipe turns at most one page (a fractional accumulator over the camera's `normalized_wheel_delta`).
- **No right-click meaning in a chest** and no menu: a plain left click or tap takes. Paging emits no event (presentation state).

## Approach

### 1. Pages and the panel
- `LoadedContainerUi.page: usize` and `page_of: Option<Entity>`: the page is treated as 0 whenever `page_of != active_container` (derived from state, not from a list of actions: `active_container` is written in five
  places: Open, Close, the Despawn arm, the LoadScene arm and the loader's reset). Re-opening the same container keeps its page.
- Cells: each of the `columns * rows` cell nodes gets `ContainerCellMarker { cell }` (the label child keeps `ContainerSlotMarker`) and becomes a `Button` (40 px minimum for the footer buttons; the 52 px cells are fine). A pure function
  `page_slot(page, cells, cell) -> usize` maps a cell to a container slot. `container_ui_system` draws labels, icons and the empty/dead background through it (guarded writes, nothing runs without an open container).
  Zero `columns`/`rows` is clamped to 1 at load (and is a `validate` error).
- Footer: `<` and `>` plain `Button`s (ASCII glyphs; they play the same click sound as other buttons if plain buttons do) and a label `1/3`; all `Without<>`-filtered against the slot label `Text` query (no B0001).

### 2. One system owns the panel's clicks: `container_click_system`
`EmitSet::Ui`, chained after `bar_assign_click_system`, reads `SimClock` (gated in the body, in the schedule test's pause-gated list), ignored while the inspector is on and while assign mode is on. It owns: the footer buttons, the wheel
(read every frame so old messages never flip a page later, applied only while a marked node is hovered), and a left click on a **filled cell**: it writes the handoff `(item_key, slot, count)` into `LoadedContainerUi` and emits
`ui.button_pressed:take_item:{item_key}`. It writes only `LoadedContainerUi.page`, the handoff and `UiEvent`s.

### 3. Taking a stack: `Action::TakeFromContainer { item_key, count (default 1), slot: Option<u32> }`
- Built **on `transfer_item`** (container spawn id -> `"player"`: it already works out the room, runs `plan` for exactly what fits and credits coin items to their stat), extended with an optional source slot
  (a new `TxnOp::TakeFrom { inv, slot, item, count }`; `TxnOp::Take` and its many call sites stay as they are). `TakeAllFromContainer`'s code is **not touched** (it stays the documented partial exception); only the "emit
  `container.looted:{id}` when the container is now empty" logic is pulled into one shared helper both actions call.
- Events: `inventory.transferred:...` / `inventory.full:player` as `TransferItem`; `container.looted:{id}` when it empties (the lootable-corpse loop depends on it, and the panel closes); `container.take_failed:{item_key}` when no
  container is open or it holds none of the item. A full bag moves nothing or part and says "Bag is full!" over the player (plain ASCII, like Buy's).
- The executor re-checks that the slot still holds the item (a rule's Take All queued earlier in the same drain runs first). Default pusher: a new prefix `take_item:` in `DEFAULT_BACKED_PREFIXES`; the handoff is taken even when a rule
  owns the event (like `menu_action`); an authored `take_item:` button with no handoff takes `count: 1` from the last stack. In local co-op the take goes to player 1's bag.
- **Take All becomes default-backed** (the pusher pushes `TakeAllFromContainer` for `ui.button_pressed:take_all_from_container` unless a rule binds it): today `take_item` would work with no rule while Take All needs one (docs/20 ~5155).

### 4. `ironhold validate`
Errors: a `ContainerPanel` (and `InventoryPanel`) with `columns` or `rows` of 0; a prefab `inventory.initial_items` that cannot fit its runtime size (`max_slots.max(4)`, counted with core's `add_to_slots` against the real catalog so it matches
spawn exactly; message names the prefab and says "needs N slots, max_slots is M"); `TakeFromContainer.item_key` through the unknown-item checks. Reachability: the reverse check's `ContainerPanel` arm inserts `take_item:{key}` for every item and
`take_all_from_container` is reachable by default. `--strict`: a container panel of fewer than 4 cells. No "panel smaller than the container" error: pages make it legitimate.

### 5. Demo and docs
`chest_panel` (3x3) now pages through `chest_swords` (20 slots = 3 pages) and `chest_01` (9) shows a hidden footer at the same panel height; the stale comments on `chest_swords` (`prefabs.ron`) and `state_machine.ron` are
rewritten. Docs: `docs/20` ContainerPanel (pages, the footer, the wheel, click-to-take, one panel height, `max_slots` vs cells, faded cells on the last page, a 4-slot corpse in a 3x3 panel now shows 5 faded cells, the co-op note,
keyboard/gamepad cannot page or take, "for feedback listen to `inventory.added:player:{item}:{count}`; bind `take_item:` only to replace the take", Take All is default-backed), `docs/30` (`container.looted` is also fired by `TakeFromContainer`, the new
action and events, clicking the last stack of a corpse loots it and closes the panel), `docs/15`, `STATUS.md`, `docs/dev/lootable-corpse.md`, CLAUDE.md files, the tests table; check whether a baseline screenshot shows an open container.

### Events and RON
No new RON fields. New: `Action::TakeFromContainer`, `container.take_failed:{item_key}`, default-backed `take_item:{item_key}` (and `take_all_from_container` default-backed).

## Tasks
- [ ] Failing tests first: `page_slot` and the dead cells on the last page; the page count follows capacity and the player stays on page 3 after taking its last items; the footer reserved and hidden at one page; `<`/`>` and the wheel
      (one notch = one page, only while a marked node is hovered, never while paused/inspector/assign mode); the camera does not zoom while a marked node is hovered and still zooms elsewhere; the page resets from state (open another
      container, close, `LoadScene`, despawn); a click takes exactly the clicked slot (slot 0 while slot 18 holds the same item: slot 18 unchanged), partial when the bag is nearly full ("Bag is full!", `inventory.full:player`), a coin item
      credits its stat, `container.looted` when it empties and the corpse flow still closes the panel, `container.take_failed`, the executor re-check after a Take All in the same drain; Take All moves hidden pages and is default-backed
      (a bound rule wins); a rule on `take_item:` replaces the built-in and the handoff is taken anyway; `validate` errors and the strict warning
- [ ] `LoadedContainerUi.page`/`page_of`, `page_slot`, `ContainerCellMarker`, the footer, `container_ui_system` through the page, `container_click_system` (+ schedule test membership and the pause-gated list), `CapturesWheel` and both camera systems
- [ ] `Action::TakeFromContainer`, `TxnOp::TakeFrom`, `transfer_item`'s optional source slot, the shared looted helper, the `take_item:` prefix and handoff, Take All default-backed (schema, every exhaustive match, executor)
- [ ] `ironhold validate` + `validate_cross_file` tests; demo comments; docs; baselines check
- [ ] Reviews, full core test loop, `cargo check -p ironhold_cli`, `cargo test -p ironhold_cli`, `ron_lint`/`ron_validation`

## Out of scope
Gamepad/keyboard container navigation, drag-out, resizing the panel to fit, scroll bars, a count picker, sorting, "take all of this item", per-player containers.

## Open questions
None blocking.

## Acceptance criteria
- Given `chest_swords` (20 slots) in a 3x3 panel, then the footer shows `1/3`; `>` shows slots 9-17, `>` again slots 18-19 with the remaining cells faded; the wheel pages one page per notch and the camera does not zoom; `chest_01` (9 slots) shows the same panel height with the footer contents hidden.
- Given slots 0 and 18 both holding iron swords, when the player clicks cell 0, then only slot 0 is emptied; taking the last items on page 3 keeps the player on page 3.
- Given a nearly full bag, when the player clicks a stack, then what fits moves, the rest stays, "Bag is full!" shows and `inventory.full:player` fires; `container.looted:{id}` fires when the container becomes empty.
- Given a rule on `ui.button_pressed:take_item:iron_sword`, then only the rule runs; Take All works with no rule.
- `ironhold validate` rejects a zero-sized panel and `initial_items` that cannot fit.

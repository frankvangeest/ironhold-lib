# Feature: Panel open-state as a set (item system S1)

_Status: Ready_
_Planned at: `594c702` (2026-11-14)_

Slice S1 of `planning/features/item_system_review.md` (the architect's item system review; Frank's answers to its nine
questions are recorded there). Small, no schema change, no RON change.

## What
The engine tracks which UI panels (inventory, shop, container) are open so that world input (F-interact, walk-over pickup,
Tab targeting) is suppressed while a *modal* panel is open. Today that state is a counter, `LoadedInventoryUi.panels_open`.
It drifts: every Close action subtracts even when its panel is not open, and `OpenInventory` always adds. The result is
interact working under an open panel, or staying dead until the next scene load. This slice replaces the counter with an
idempotent set, and makes the inventory panel non-modal (Frank, 2026-11-14: an open inventory must not block interaction;
the shop and container panels still do).

## Why
Found during the 2026-11-14 playtest: with the inventory open, F at the `seal_door` gate did nothing, silently. Also the
foundation for the rest of the item system (S2-S6 add more panel verbs and a context menu, which would multiply the drift).

## Approach
- `PanelKind { Inventory, Shop, Container }` (`Ord`), and `LoadedInventoryUi.open_panels: BTreeSet<PanelKind>` replace
  `panels_open: u8` (`capabilities/inventory.rs`). Same lesson as `SimClock`'s hold set: Open inserts, Close removes, a Close
  of a panel that is not open is a no-op, `LoadScene` clears it. No hash container, so determinism-lint is untouched.
- One helper, `world_input_blocked()`, true when a modal panel (`Shop`, `Container`) is open; `Inventory` is non-modal.
  The three gates (`interactable_system`, `collectible_system`, `tab_targeting_system`) read it, each with the `debug!` line
  from the existing backlog item "Silent `panels_open > 0` early-return gate ... logs nothing", saying which panel blocks.
- Executor arms: `OpenInventory`/`CloseInventory`/`ToggleInventory`, `OpenShop`/`CloseShop`, `OpenContainer`/`CloseContainer`,
  the `Despawn` teardown of an open container, and `LoadScene` use the set. The existing "already open" guards on
  `OpenShop`/`OpenContainer` become unnecessary for counting but keep their re-target behaviour.
- `ui.panel_opened` / `ui.panel_closed` events keep today's emission (unchanged) so no project's RON changes meaning.
- Unchanged on purpose: the action bar is not gated by this state (the note in `crates/ironhold_core/src/CLAUDE.md` about
  keeping the check separate from the pause gate stays true).

## Tasks
- [ ] `PanelKind`, `open_panels`, `world_input_blocked()`; remove `panels_open`/`set_panel_open`
- [ ] Executor arms and the `Despawn` teardown use the set; `LoadScene` clears it
- [ ] The three gates use `world_input_blocked()` plus a `debug!` each
- [ ] Update the existing tests that read `panels_open` (`action_tests.rs`, `corpse_loot_interact_tests.rs`)
- [ ] New tests (written first, red against the counter): OpenInventory twice then one Close leaves it closed; Close of a
      panel that is not open changes nothing; the walk-past case (inventory open, shop opened and closed twice) keeps the
      inventory in the set; inventory open does not block `interactable_system`, shop/container open does
- [ ] Docs: `crates/ironhold_core/src/CLAUDE.md`, `runtime/scene_manager/CLAUDE.md` (two sections), `tests/CLAUDE.md`,
      `docs/30_runtime_events_and_logic.md` / `docs/20_data_formats.md` wherever the counter or "panel blocks interact" is described
- [ ] Reviews (alignment, architect, detective), full core test loop, `cargo check -p ironhold_cli`

## Open questions
- None blocking. (The inventory-modality question was answered 2026-11-14.)

## Acceptance criteria
- Given the inventory open, when the player presses F inside a gate's radius with the key, then the gate opens.
- Given the inventory and then the shop open, when the shop closes twice (button, then leaving the merchant zone), then the
  inventory is still open and no state is negative or stuck.
- Given `OpenInventory` called twice, when `CloseInventory` is called once, then no panel is open.
- Given the shop or a container open, then interact, walk-over pickup and Tab targeting stay suppressed, and each logs at
  `debug` which panel blocks it.

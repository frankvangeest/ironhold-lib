# Feature: Inventory / shop / container input blocking

_Status: In Progress_
_Planned at: `bd810c3` (2026-06-28)_

## What

While an inventory, shop, or container panel is open:
- Base-scene UI buttons must not respond (dance button, etc.) — only within the panel's screen rect
- World-space targeting (left-click and Tab-cycle) must not fire within the panel rect
- F-key interactable and collectible walk-overs are suppressed everywhere (keyboard/physics — not position-based)

## Why

There are two independent click paths in Bevy 0.18. Neither `GlobalZIndex` alone nor an
action-time-spawned blocker node can cover both:

1. **UI buttons** go through `ui_focus_system` → `Interaction` → `button_system`. Only a node
   with a computed layout (`ComputedNode`) participates. A node spawned via deferred `Commands`
   at action time has no layout until `PostUpdate` — too late for `ui_focus_system` to pick it
   up on that frame.
2. **World targeting** reads `ButtonInput<MouseButton>` directly in `click_select_system` and
   `tab_targeting_system` — it never touches `ui_focus_system`. A UI node of any kind cannot
   block this path; only an explicit `panels_open` counter guard in those systems can.

## Approach (final — per-rect panel blocking)

### UI click blocking — `FocusPolicy::Block` + `Interaction` on panel roots

Each panel root node (InventoryPanel, ShopPanel, ContainerPanel) at `GlobalZIndex(99)` carries
`Interaction::default()` and `bevy::ui::FocusPolicy::Block`.

How `ui_focus_system` works in two loops:
- **Loop 1**: builds `hovered_nodes` — every node whose `ComputedNode` rect contains the cursor
  and has `inherited_visibility == true`, in z-order.
- **Loop 2**: iterates top→bottom. Sets `Interaction` if the node has it, then checks
  `FocusPolicy`: `Block` → stop; `Pass` → continue.

Result: clicks **within a panel rect** are absorbed by the panel root (or by a child button above
it in z). Clicks **outside the panel rect** — e.g. the dance button in a non-overlapping area of
the screen — proceed normally through the stack.

`FocusPolicy::Block` on the panel root does **not** prevent its own child buttons from receiving
events: children sit above the root in z-order, so Loop 2 reaches them first before the root's
Block fires.

`Interaction::default()` on the root is required to handle dead-space clicks (inside the panel
rect but not on a child button): without it, nothing registers Pressed and the
`click_select_system` guard fails to fire, targeting a world entity behind the panel.

### World targeting blocking — `panels_open: u8` counter guard

`panels_open: u8` on `LoadedInventoryUi` counts open panels. Tab-cycle, F-key interactable,
and collectible walk-over all guard on `panels_open > 0` (keyboard/physics input is not
position-based so a flat block is correct).

`click_select_system` uses a different guard: `ui_interactions.iter().any(Pressed)` — which
fires whenever a panel root or child button gets Pressed by `ui_focus_system`. This means world
targeting is suppressed exactly when a click lands inside a panel rect.

### Deliberate behavior change vs. original full-screen blocker

The original approach used a hidden full-screen `Node` at `GlobalZIndex(98)` that was made
visible on any Open action. This blocked all clicks everywhere, including areas of the screen
that don't overlap any panel. The per-rect approach intentionally removes that suppression:
clicking empty world space outside an open panel now proceeds normally (e.g. clearing the current
target). This is the correct behavior — frank's explicit requirement.

### No new RON fields or actions

`OpenInventory`/`CloseInventory`/`OpenShop`/`CloseShop`/`OpenContainer`/`CloseContainer` RON
signatures are unchanged. `ui.panel_opened` and `ui.panel_closed` `GameEvent`s are emitted for
optional designer hooks in rules.ron.

## Implementation

### `capabilities/inventory.rs`
- `panels_open: u8` on `LoadedInventoryUi` (replaced the previous `panel_open: bool`)
- `panel_blocker: Option<Entity>` removed — no longer needed

### `runtime/scene_manager/scene_loader.rs`
- InventoryPanel, ShopPanel, ContainerPanel roots each get `Interaction::default()` + `bevy::ui::FocusPolicy::Block` at spawn time
- Full-screen Panel Blocker spawn block removed

### `runtime/scene_manager/action_executor.rs`
- All 6 Open/Close arms + ToggleInventory: only update `panels_open` counter; no blocker entity manipulation
- LoadScene arm: resets `panels_open = 0`; no `panel_blocker = None`

### `capabilities/targeting.rs`
- `click_select_system`: `ui_interactions.iter().any(Pressed)` guard — mechanism unchanged, now
  driven by panel roots instead of the blocker entity
- `tab_targeting_system`: `panels_open > 0` guard

### `capabilities/interactable.rs`, `capabilities/collectible.rs`
- `panels_open > 0` guard

### Tests
- `crates/ironhold_core/tests/ui_panel_blocker.rs` — 2 regression tests verifying `FocusPolicy::Block`
  stops focus iteration; `FocusPolicy::Pass` does not

## Acceptance criteria

- Given inventory is open, clicking the dance button where it does NOT overlap the panel fires normally.
- Given inventory is open, clicking inside the panel rect fires no base-scene action.
- Given inventory is open, clicking a monster/chest behind the panel does not change target.
- Given inventory is open, Tab-cycling does not change target.
- Given inventory is open, pressing F near an interactable fires no event.
- Given inventory is open, walking over a collectible does not collect it.
- Given the panel is closed, all interactions resume immediately.
- Shop and container panels have identical blocking behaviour.

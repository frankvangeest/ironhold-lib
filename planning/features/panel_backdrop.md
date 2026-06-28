# Feature: Inventory / shop / container click-blocking backdrop

_Status: In Progress_
_Planned at: `bd810c3` (2026-06-28)_

## What

When `OpenInventory`, `OpenShop`, or `OpenContainer` shows a panel, a transparent full-screen backdrop node becomes visible beneath the panel, blocking pointer events and world-space interactions (interactable F-key, collectible walk-over) from firing while the panel is open. Disappears when the last open panel closes.

## Why

Without this, NPC interactable prompts and collectible pickups fire through the panel UI while it is open. The overlay backdrop (just shipped) solved the same problem for overlay scenes; this solves it for in-scene panels that toggle visibility rather than load as overlay scenes.

## Approach

### Z-index layering

| Layer | GlobalZIndex | Entity type |
|---|---|---|
| Base-scene UI | 0 (default) | LevelEntity root + children |
| Panel backdrop | 50 | LevelEntity, Visibility::Hidden |
| Panel entities (inventory/shop/container) | 51 | Children of UI root, with GlobalZIndex override |
| Overlay backdrop | 100 | OverlayEntity |
| Overlay content | 101 | OverlayEntity |

Panel entities are children of the UI root, but adding `GlobalZIndex(51)` to them makes Bevy's stack system treat them as z-indexed root nodes, rendering above the backdrop at 50.

### Backdrop entity lifecycle

The backdrop is spawned lazily during scene load — the first `InventoryPanel`, `ShopPanel`, or `ContainerPanel` node encountered in `spawn_ui_element_node` spawns it (if not already spawned) and stores the entity in `LoadedInventoryUi.panel_backdrop`.

Default: `Visibility::Hidden`. On open action → `Visibility::Visible`. On close action → `Visibility::Hidden` (panels are exclusive in practice — only one open at a time).

### World-space interaction blocking

`LoadedInventoryUi` gains a `panel_open: bool` field. The executor sets it on open/close. Two systems gain an early-return guard:

- `interactable_system` — skips F-key emit if `panel_open`
- `collectible_system` — skips pickup emit if `panel_open`

### No new RON fields or actions

`OpenInventory`/`CloseInventory`/`OpenShop`/`CloseShop`/`OpenContainer`/`CloseContainer` are unchanged in RON signature. The backdrop is a silent implementation detail.

## Tasks

- [ ] `capabilities/inventory.rs` — add `panel_backdrop: Option<Entity>` and `panel_open: bool` to `LoadedInventoryUi`
- [ ] `scene_loader.rs` — in `InventoryPanel`, `ShopPanel`, `ContainerPanel` spawn branches: add `GlobalZIndex(51)` to panel entity; spawn backdrop lazily and store in `LoadedInventoryUi`
- [ ] `action_executor.rs` — in Open/Close arms: show/hide backdrop, set `panel_open`
- [ ] `capabilities/interactable.rs` — early return if `panel_open`
- [ ] `capabilities/collectible.rs` — early return if `panel_open`
- [ ] Tests
- [ ] Docs: `docs/20_data_formats.md` — note backdrop in panel UI node entries

## Open questions

- Should collectible walk-over be blocked? Backlog says yes — consistent with "no world interactions while panel is open."

## Acceptance criteria

- Given inventory is open, when player presses F near an interactable NPC, the interaction does not fire.
- Given inventory is open, when player walks over a collectible, it is not collected.
- Given inventory is open, clicking outside the panel does not trigger base-scene buttons.
- Given the panel is closed, all world interactions resume immediately.
- Panel buttons (close, buy, take) work normally — the backdrop does not block the panel itself.

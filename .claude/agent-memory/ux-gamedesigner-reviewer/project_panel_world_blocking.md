---
name: project_panel_world_blocking
description: Which item panels block F-interact/pickup/Tab (shop+container yes, inventory no since item_panels_open_set); ui.panel_opened/closed carry no panel name and are undocumented
metadata:
  type: project
---

Since feature `item_panels_open_set` (reviewed 2026-11-14): shop and container panels suppress
F-interact, walk-over pickup and Tab targeting; the inventory panel does not. Not configurable
(no "modal inventory" option).

Open designer gaps found at review time:
- Rule documented only as a bullet inside docs/30's lootable-corpse "Key notes" (~1074); the
  panel reference (docs/20 InventoryPanel/ShopPanel/ContainerPanel ~1358-1450) and the
  `interactable`/`targetable` prefab rows (docs/20 ~2062-2064) say nothing.
- `ui.panel_opened` / `ui.panel_closed` are emitted by all six open/close actions (and by Despawn of
  an open container) but have no `:<panel>` suffix and are absent from docs/30's event list.
  Trap: closing the bag while a shop is open fires `ui.panel_closed` although a blocking panel is
  still open.
- Dialogue panel's blocking behaviour and co-op scope (one player's shop blocks everyone?) unstated.
- docs/30 ~1071 cites the Rust type `LoadedInventoryUi.open_panels` (see [[project_ron_comments_cite_dev_paths]]).

**How to apply:** on any panel/interact/pickup/targeting change, check these surfaces stay in sync.

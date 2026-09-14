---
name: inventory-panel-triplication
description: Inventory/Shop/Container panels are three hand-synced copies of the same chrome in scene_loader.rs; their UI systems are singleton-by-assumption with no duplicate-panel guard, and two slot-tree shapes diverge
metadata:
  type: project
---

The three item-UI panels (`UiNodeDef::InventoryPanel` / `ShopPanel` / `ContainerPanel` arms of
`spawn_ui_element_node` in `runtime/scene_manager/scene_loader.rs`) are near-identical copy-paste
constructors, kept in sync by hand.

**Shared chrome that exists in triplicate:** header row Node (`SpaceBetween` + `width 100%` +
padding), a ~35-line close-button block (22x22, 1px border, `BorderColor srgba(0.5,0.1,0.1,0.7)`,
`BackgroundColor srgba(0.22,0.05,0.05,0.85)`, catalog `ui/cross` icon with `Text "\u{2715}"`
fallback, only `UiAction::Trigger` name differs), title `TextFont { font_size: 12.0 }`, and content
area padding `8.0`. A padding drift between them was found by live playtest, not by any test
(2026-09-14) — nothing mechanically detects divergence.

**Singleton-by-assumption.** `LoadedInventoryUi.inventory_panel/shop_panel` and
`LoadedContainerUi.container_panel` are each `Option<Entity>`, last-spawn-wins, set silently with
no warning on a second panel of the same kind. `LoadedContainerUi.active_container` is a single
`Option`, and `mod.rs`'s `container_panel_q`/`inventory_panel_q`/`shop_panel_q` toggle *every*
matching entity's `Visibility`. So `container_ui_system`'s flat `Query<(&ContainerSlotMarker, &mut
Text)>` (no `Children` scoping) is consistent with the rest of the design, not a new risk — but
neither `ironhold_cli validate` nor scene load warns on a duplicate panel node.
`LoadedContainerUi.container_panel` is currently **written and never read** — the scoping handle a
future multi-panel design would need already exists but is dead.

**Two divergent slot-tree shapes.** InventoryPanel slot = marker node + child `SlotIcon` +
child `SlotLabel` carrying `InventorySlotLabelMarker`, explicitly commented "spawned after icons so
it renders on top". ContainerPanel slot = `ContainerSlotMarker` + `Text` **on the slot node itself**
+ an absolutely-positioned icon child — so in Bevy's pre-order UiStack the count text paints *under*
its own icon. Also `ContainerPanelDef` has no `initially_hidden` (hardcoded `Visibility::Hidden`)
and no `title` (hardcoded literal), unlike the other two defs.

**How to apply:** any further panel work should extract a shared
`spawn_panel_header(parent, title, close_trigger, tint, close_icon)` helper plus shared chrome
constants before adding a fourth panel, and converge container slots onto the label-child pattern.
Treat a duplicate-panel-node check as a `validate` + scene-load-`warn!` mirror pair
(see [[cli-runtime-mirror-check-pairs]], [[ui-flat-scan-and-taffy-facts]]).

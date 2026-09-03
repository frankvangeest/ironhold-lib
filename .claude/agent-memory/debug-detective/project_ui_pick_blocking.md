---
name: project-ui-pick-blocking
description: How UI click-blocking works in this Bevy 0.18 project — coverage + z-order, not Button/Interaction presence; targeting bypasses UI picking entirely
metadata:
  type: project
---

CORRECTION (supersedes the original claim below): this project's UI buttons are driven by
the **`ui_focus_system` / `Interaction` path**, NOT the bevy_picking observer path.
`button_system` (lib.rs) reads `Changed<Interaction>` + `With<Button>`, and `Interaction` is
set by `bevy_ui::focus::ui_focus_system`. In that path, a higher-z node does NOT block lower
nodes by default — it blocks only if it carries `FocusPolicy::Block`, and `Node`'s required
`FocusPolicy` defaults to **`Pass`**. See [[focuspolicy-block-required]] for the proven
root cause and the headless regression test. The original text below (picking-backend
`should_block_lower` reasoning) describes a DIFFERENT mechanism and was not what governed the
dance-button click-through bug.

--- original (picking-observer path; verify before relying on it) ---

In Bevy 0.18 (DefaultPlugins, bevy_ui picking backend on), UI `Node` entities are pickable by default with `Pickable { should_block_lower: true }`. A plain `Node` (no `Button`/`Interaction`) DOES block picks to lower-z nodes it spatially covers. The base button's `Interaction` is driven by the picking backend (`button_system` in lib.rs reads `Changed<Interaction>`).

**Two consequences that bite the inventory-panel feature:**

1. UI-vs-UI blocking is governed by (a) spatial coverage at the cursor and (b) `GlobalZIndex`. The overlay-backdrop pattern (`scene_loader.rs` ~line 1196) works because it is `width/height: 100%` full-screen — NOT because it has any special component. A panel sized from `el.size()`/`el.position()` (InventoryPanel/ShopPanel/ContainerPanel) only blocks the rectangle it covers; a base button outside that rectangle stays clickable even with `GlobalZIndex(51)`. Fix = full-screen blocker node, not just z-index on the panel.

**Why:** GlobalZIndex alone only resolves draw order / pick priority where nodes overlap; it does not extend the panel's pickable area.

2. World-space targeting (`capabilities/targeting.rs` `click_select_system`) reads `mouse.just_pressed(Left)` directly via `Res<ButtonInput<MouseButton>>` and projects entities with `world_to_viewport`. It does NOT go through bevy_picking/UI at all, so NO amount of UI z-index or blocker node will stop it on its own. It needs an explicit guard against a click that a UI panel already consumed — see below for how that guard actually works today.

**FIXED, via a different mechanism than originally prescribed.** `click_select_system` does not
gate on a `panel_open` resource flag. Instead it takes `ui_interactions: Query<&Interaction>` and
early-returns when `ui_interactions.iter().any(|i| *i == Interaction::Pressed)` (targeting.rs
~line 187/196). This works because every panel root carries `FocusPolicy::Block` + `Interaction`
(see [[project_focuspolicy_block_required]]) — `ui_focus_system` sets that root's (or its child
button's) `Interaction` to `Pressed` for any click landing inside the panel's rect, so this check
transitively covers "a click was consumed by any open UI panel or button" without needing a
separate per-panel `panel_open` resource at all. A click outside every panel's rect leaves all
`Interaction`s un-`Pressed` and falls through to world targeting normally.

**How to apply:** if a new UI panel/blocker needs to also block world-space targeting, giving it
`FocusPolicy::Block` (so it participates in `ui_focus_system`'s `Interaction` resolution) is
sufficient — no additional wiring in targeting.rs is needed. `LoadedInventoryUi.panels_open` (a
u8 refcount, not a bool) still exists and still gates `collectible_system`/`interactable_system`
(a resource-refcount check, not the `Interaction`-query check `click_select_system` uses) — the
two mechanisms are not unified, so don't assume one implies the other is also guarded. See
[[project_panels_open_refcount_leak]] for a real bug in that refcount's own bookkeeping.

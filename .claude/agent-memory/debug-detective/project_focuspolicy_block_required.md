---
name: focuspolicy-block-required
description: A full-screen UI blocker node needs FocusPolicy::Block to stop click-through; GlobalZIndex alone does not block in Bevy 0.18 ui_focus_system
metadata:
  type: project
---

In Bevy 0.18, a higher-`GlobalZIndex` full-screen UI node does NOT stop clicks from
reaching lower UI nodes on its own. `ui_focus_system` (bevy_ui/src/focus.rs) walks the
hovered nodes top-to-bottom and only STOPS when it hits a node whose `FocusPolicy ==
Block`. The catch: `Node` `#[require]`s `FocusPolicy`, and `FocusPolicy::DEFAULT` is
**`Pass`**, not `Block`. The `node.focus_policy.unwrap_or(&Block)` in focus.rs is dead
code for UI nodes because the component is always present (defaulting to Pass). So a
blocker node without an explicit `FocusPolicy::Block` lets the focus iteration continue
and ALSO press the button beneath it.

**Why:** This was the root cause of the inventory/shop/container panel click-through bug
(dance button firing through an open panel). The panel blocker had `GlobalZIndex(98)` +
`Interaction::default()` + correct full-screen layout + correct visibility — everything
looked right, but it defaulted to `FocusPolicy::Pass`. Fix: add `bevy::ui::FocusPolicy::Block`
to the blocker node in scene_loader.rs ("Panel Blocker" spawn).

**How to apply:** Any time a UI node must absorb/block pointer events for nodes beneath it
(modal backdrops, input blockers, full-screen overlays), it MUST carry
`FocusPolicy::Block`. Z-index only orders the stack; it does not block. Reachable panel
buttons must sit at a HIGHER GlobalZIndex than the blocker (panels at 99 > blocker 98) so
they are hit before the blocker stops iteration.

**Note on the overlay backdrop:** the z=100 "Overlay Backdrop" has NO `Interaction` and NO
`FocusPolicy::Block`, yet appears to "work" — it likely blocks only incidentally because
the pause-overlay panel/content above it captures the click, not because the backdrop
itself blocks. If a base-scene button is ever reachable under an overlay, give the backdrop
`FocusPolicy::Block` too. See [[ui-pick-blocking]].

**Verified empirically:** `crates/ironhold_core/tests/ui_panel_blocker.rs` runs the real
Bevy UI focus pipeline headlessly (MinimalPlugins + UiPlugin + DefaultPickingPlugins +
TextPlugin, camera with explicit `viewport`, a PreUpdate system forcing InheritedVisibility
since there is no render view, and an inject_click system ordered after InputSystems /
before UiSystems::Focus). With Pass the button reads `Pressed`; with Block it reads `None`.

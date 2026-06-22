---
name: ui-hover-and-tooltip
description: Bevy UI hover detection, z-order, cursor/viewport access, and the tooltip_system design constraints in ironhold-lib
metadata:
  type: project
---

UI hover/focus + tooltip infrastructure facts (verified at a6acab8, 2026-06-22).

**Hover detection.** The canonical pattern is `button_system` in `lib.rs` — `Query<(&Interaction, ...), (Changed<Interaction>, With<Button>)>`. `Interaction` only exists on entities that carry Bevy's `Button` component.
- Action bar slots (`ActionSlotUi`, scene_loader.rs ~line 1773) DO have `Button` → `Interaction` is available for free.
- Inventory slots (`InventorySlotMarker`, scene_loader.rs ~line 1992) do NOT have `Button` — they need a `Button` (or `Interaction`) component added before hover detection works. This asymmetry is the main wiring gap for any hover feature on inventory.

**Cursor + viewport access (WASM-safe).** `Query<&Window, With<PrimaryWindow>>` then `window.cursor_position()` and `window.width()/height()`. Established in `targeting.rs` (click_select_system). Bevy abstracts the windowing layer, so this works identically native + WASM — no platform API. Use `window.size()` for viewport bounds when edge-clamping a tooltip.

**UI camera + z-order.** UI renders on a `Camera2d` at `order: 1000` (lib.rs ~line 320, `clear_color: None`). Within that camera, sibling paint order determines stacking (later children render on top); for cross-hierarchy "always on top" use Bevy `GlobalZIndex`. A tooltip overlay should be a root-level UI node with a high `GlobalZIndex` so it floats above panels regardless of spawn order.

**ActionSlotDef.label.** Field already exists (scene_v2.rs ~line 836), `Option<String>`, documented "Optional tooltip label shown on hover (future use)" — declared but never rendered anywhere. Any tooltip feature should resolve this: either repurpose `label` as the tooltip title or supersede it with a structured `tooltip`/`description` field. Do not leave two overlapping fields undocumented.

**No tooltip infrastructure exists** as of this date — no overlay entity, resource, or hover-tracking system beyond `button_system`'s colour swap.

**Designer-authored, not auto-generated.** Tooltip content (skill description, do_actions summary) must be authored in RON, never reflected/auto-generated from the `Action` list — auto-generation would couple UI text to internal action variant names and break the data-driven contract.

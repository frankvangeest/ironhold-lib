---
name: icon-sheet-empty-string-sentinel
description: ActionBarDef.icon_sheet treats Some("") as "unset" at runtime (explicit .filter), but InventoryPanel/ContainerPanel/ItemDef do not — a texture-key validator must special-case the ActionBar
metadata:
  type: project
---

`scene_loader.rs`'s action-bar slot atlas resolution is
`bar.icon_sheet.as_deref().filter(|s| !s.is_empty())` — an empty string is a *supported* "no sheet"
sentinel, deliberately matching `ActionSlotDef.icon`'s documented "leave empty to use the bar-level
sheet". `InventoryPanelDef.icon_sheet` / `ContainerPanelDef.icon_sheet` have **no** such filter:
they push `""` into the sheet-key list, `textures.get("")` misses, and the panel just gets no
atlas (silently).

**Why:** a `check_texture_key(...)`-style validator that fires on any `Some(key)` turns
`icon_sheet: ""` on an ActionBar into a hard exit-1 on a scene that renders correctly. Found in the
cli_validate_batch3 review (2026-09-11), proven with a one-node fixture.

**How to apply:** guard texture-key checks with `.filter(|s| !s.is_empty())` wherever the runtime
does. `FoliageMaterialDef.leaf_texture` already has its own empty-string guard for the same reason
— that guard is precedent, not an unexplained quirk.

Sibling still uncovered as of that batch: `IconButtonDef.icon_on`/`icon_off` are required
`AssetCatalog.textures` keys resolved with `.unwrap_or_default()` and **zero** runtime warning
(`scene_loader.rs` ~1781) — the exact failure shape the batch's own doc comment describes.

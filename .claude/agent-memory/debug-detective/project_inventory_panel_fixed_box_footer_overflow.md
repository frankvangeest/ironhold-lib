---
name: inventory-panel-fixed-box-footer-overflow
description: InventoryPanel/ContainerPanel outer box is a fixed px size from UiNodeDef::size(); footer text that outgrows it wraps/spills outside the background, no test sees it
metadata:
  type: project
---

`UiNodeDef::size()` (schema/scene_v2.rs) gives InventoryPanel an explicit pixel box: width
`columns*(slot_size+slot_gap)+slot_gap+16`, height `rows*(...)+slot_gap+68` (a fixed ~28px budget
for the footer row). Children are not measured: anything added to the header/footer that is wider
than the box makes Bevy text nodes shrink to min-content and wrap (footer row grows past the 28px
budget, spills below the panel background) or overflow sideways past the right edge.

Seen 2026-11-15 reviewing feature/player_inventory_slots: the new "Slots: used/total" label beside
"Gold: N" (justify SpaceBetween, no column_gap) fits the default 5-column panel easily but not a
1-column panel (inner ~60px at default 48px slots) and is tight at 2 columns (~112px inner).
"Gold: 0" alone used to fit a 1-column panel.

**Why:** no test spawns a real InventoryPanel through scene_loader and inspects layout; only
the browser shows it, and the only shipped panel (3rd_person_game_demo) is 5 columns wide.
**How to apply:** for any new text in panel headers/footers, check it against the narrowest
legal columns value and the font_size field, not the demo panel. Related: [[embedded-font-is-ascii-only]].

---
name: inplace-confirm-double-click
description: The S4 bag slot menu swaps its verb rows for Prompt/Yes/No in the same overlay; a top-anchored menu puts Yes where Destroy was, so a double-click confirms
metadata:
  type: project
---

`capabilities/slot_menu.rs` (item system S4, reviewed 2026-11-15) shows its Yes/No confirm by hiding the verb rows and showing
Prompt/Yes/No in the same node. When the menu opens downward (top-anchored, upper half of the grid), rows stack from the top, so
with no shop open and a usable item, Destroy (Header+Use+Destroy, ~74-114px) and Yes (Header+Prompt+Yes, ~60-118px) overlap. A
double-click on Destroy is then a confirmed destroy. The same applies to "Sell N" on a non-usable stack with a shop open. When the
menu opens upward (bottom-anchored), the last row in both modes is Destroy and then No, so it is safe there.

**Why:** a confirm step exists to make a destructive verb take two separate decisions. Bevy's press-edge rule (a fresh press is
needed, see [[bevy-interaction-press-edge-semantics]]) does not help, because the second click of a double-click is a fresh press.

**How to apply:** for any in-place confirm (same overlay, rows swapped), check row geometry in both anchor directions. The safe
designs are: No in the slot where the verb was, Yes far from it, or ignore presses for a short time after the confirm appears.
Integration tests cannot see this (the test harness has no layout), so it needs a playtest or a geometry check by reading the code.

---
name: panels-open-refcount-leak
description: OpenContainer/OpenShop unconditionally increment LoadedInventoryUi.panels_open, so two interactables in range on the same frame permanently disables interact + pickup + tab-targeting
metadata:
  type: project
---

`LoadedInventoryUi.panels_open: u8` is a refcount incremented by every `Open*` action and
decremented by every `Close*` action (`capabilities/inventory.rs`, `set_panel_open`). `Action::OpenContainer`
and `Action::OpenShop` increment **unconditionally** — there is no "already open" guard
(`action_executor.rs`). Only `Action::LoadScene` resets it to 0.

`interactable_system`, `collectible_system`, and `tab_targeting_system` all early-return on
`panels_open > 0`. So one leaked increment silently kills interact, item pickup, and Tab targeting
for the rest of the scene.

**The reachable double-open:** `interactable_system` loops over *all* `Interactable` entities within
radius and emits `entity.interacted:{id}` for **each** — no nearest-only dedup. If two RON handlers
both answer with `OpenContainer`, both run in the same `ActionQueue` drain: `panels_open` goes to 2,
one `CloseContainer` brings it to 1, and it never returns to 0.

**Why:** this was unreachable while every `Interactable` was static and hand-placed far apart
(chest_01 at (-2.5,0.4,10.5) vs merchant_01 at (-8,0,10) — 5.5 m, radii 2.0/3.0, can't overlap). It
becomes reachable the moment a *mobile* or *duplicated* entity gets `interactable:` — e.g. two
zombies from the same prefab that both path to the player and stop ~1 m apart.

**How to apply:** any diff that adds `interactable:` to an NPC/mobile prefab, or to a prefab
instanced more than once, is a candidate for this soft-lock. Ask: can two of these be within radius
of the player simultaneously? Related: [[gamepad-join-emit-vs-capture]] (same shape — one emitter
writing N events where the consumer assumes 1).

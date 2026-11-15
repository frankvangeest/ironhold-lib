---
name: self-vs-player-bag-address
description: Item actions address the player's bag only via the literal "player"; `{self}` rewrites to the player's spawn id (e.g. "player_01"), which AddItem/RemoveItem/TransferItem treat as a container entity
metadata:
  type: project
---

`inv_key()` in `runtime/scene_manager/item_actions.rs` (and the `AddItem` arm in `action_executor.rs`) map ONLY the
literal string `"player"` to `PlayerInventory`; any other string is looked up as a container entity's spawn id.
`rewrite_self` substitutes `{self}` in `AddItem/RemoveItem/TransferItem.entity`, so `RemoveItem(entity: "{self}", ...)`
inside an item's `use:` list (S3, where `{self}` = primary player) becomes `"player_01"`: a warn +
`inventory.remove_failed:player_01`, never the bag. Same for player-owned behaviors using `{self}`.

**Why:** found reviewing item system S3 (feature/item_use, 2026-11-15). Docs say "`{self}` is the player" in `use:`,
which invites exactly this; validate does not check item-action entity fields at all.

**How to apply:** when a change documents `{self}` as "the player" anywhere item verbs can appear, check whether the
docs/validate steer designers to `"player"` for bag operations. See [[self-target-substitution-coverage]].

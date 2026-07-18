---
name: interactable-vs-targeting-player-scope
description: interactable_system is single-player-only (single()) while tab_targeting_system is fully per-player — a divergence that bites every local-coop parity feature
metadata:
  type: project
---

`capabilities/targeting.rs` and `capabilities/interactable.rs` look like "the same pattern"
(both read an `InputMap` field directly, both bypass `InputAction`) but they diverge on
player-scope, and this trips up local-coop parity planning repeatedly.

- `tab_targeting_system` was rewritten to be **fully per-player**: it `for`-loops over every
  `(&CharacterController, .., &mut PlayerTarget)` and each player cycles its own target with its
  own `InputMap.target_next` key. Adding a per-player gamepad check here is a clean fit.
- `interactable_system` is still **single-player-only**: it does `player_query.single()` and
  early-returns the moment 2+ `CharacterController` entities exist. So in any local-coop
  (split-screen) scene, `entity.interacted:{id}` never fires for *anyone* — keyboard or gamepad.
  It also early-returns on the keyboard-key miss (`if !just_pressed { return }`) before any
  per-entity work, so a gamepad check must be restructured into a combined boolean, not appended.

**Why:** surfaced during the gamepad_controller_input.md plan review (2026-07-19). The plan
framed gamepad interact as a local-coop parity fix, but the fix as written only works in
single-player scenes because of the `single()`.

**How to apply:** when a feature touches "player presses key → per-entity effect", check whether
the target system is the per-player loop shape (targeting) or the single() shape (interactable).
Any local-coop feature that wants interact to work for 2+ players needs `interactable_system`
rewritten to a per-player loop first — treat that as an explicit scope decision, not an
implementation detail. Related: [[player_marker_gap]], [[targeting_currenttarget_mirror]].

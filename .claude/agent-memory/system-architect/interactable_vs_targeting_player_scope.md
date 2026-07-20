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
- `interactable_system` was **single-player-only** (`player_query.single()`, early-returns the
  moment 2+ `CharacterController`s exist → `entity.interacted:{id}` fired for no one in any
  local-coop scene). **Fixed on branch `fix/interactable-multiplayer` (2026-07-19):** rewritten to
  a per-player `for (transform, controller) in &player_query` loop with `hit_any` declared per
  iteration — no cross-player interference, both queries stay read-only (no borrow/query conflict).

**Why:** surfaced during the gamepad_controller_input.md plan review (2026-07-19). Fixed
independently of gamepad since the bug predates and is unrelated to it.

**Residual scope caveat (still true after the fix):** `interactable_system` uses `Transform`
(not `GlobalTransform` like tab_targeting) and emits **globally-unscoped** GameEvents. For
`entity.interacted:{id}` that's fine (event is about the entity). But `player.attack_missed` is
semantically player-scoped yet emitted globally — now fires once per player-who-pressed-and-missed
per frame. Safe today because its only consumer is `primitive_world` (single-player,
`state_machine.ron`). A future multiplayer scene that listens on `player.attack_missed` (or wants
per-player interact feedback) will need player-scoping — same class as targeting's
primary/non-primary split. Also note: `player.attack_missed` emitted from an *interact* system is
a pre-existing naming smell.

**How to apply:** interactable now matches the per-player pattern; treat the two systems as
consistent. When adding per-player interact *feedback* (not just the interact trigger), the
global `player.attack_missed`/`entity.interacted` scoping is the next thing to revisit. Related:
[[player_marker_gap]], [[targeting_currenttarget_mirror]].

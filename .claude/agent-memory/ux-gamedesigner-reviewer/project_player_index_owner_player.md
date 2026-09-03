---
name: player-index-owner-player-wiring
description: owner_player (ActionBar/SetCameraMode/CameraShake) matches a player prefab's player_index field; terminology is now cross-linked in docs (~1010, ~2514-2515); canonical shipped example below
metadata:
  type: project
---

`ActionBarDef.owner_player: Option<u32>` (and `SetCameraMode`/`CameraShake`'s `owner_player`) routes
to a player whose `PrefabDef.player_index` matches. Semantics: `None`/`Some(0)` = primary player,
`Some(n)` = the player prefab with `player_index: n`.

**CLOSED — terminology gap fixed.** Docs now explicitly cross-link the two field names at both
sites: the `ActionBarDef.owner_player` row (docs/20 ~1010) says "set to the same value as that
player's `player_index` field (`PrefabDef.player_index`, see `PrefabDef`'s field table)", and the
`SetCameraMode`/`CameraShake` `owner_player` section (docs/20 ~2514-2515) says "`n` matches the
`player_index:` field on a player prefab (see `PrefabDef` above)". Do not re-flag this as an
undocumented terminology gap.

Canonical shipped example: `local_coop_demo/room3.scene.ron` (`owner_player` 0/1, keys G/L) and
`room11.scene.ron` (`player_p1_camera_switch` at `player_index: 0`, targeted by
`SetCameraMode(mode: "cinematic_fixed", owner_player: 0)`).

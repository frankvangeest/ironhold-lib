---
name: renderlayers-reserved-scheme
description: The per-split-player RenderLayers scheme (layers 1-4) has two remaining fragility points — a warned-but-not-prevented modulo collision on player_index, and spawn-time-only application; the third (a hardcoded party-camera union literal) is fixed via all_ring_layers()
metadata:
  type: project
---

`SplitScreenDef.own_viewport_only` (added 2026-07-31) is the codebase's first designer-facing
`RenderLayers` user. Reserved layers 1-4, computed as `1 + player_index % MAX_SPLIT_PLAYERS` via
`ring_layer_for_player()` (`camera.rs`) at four independent sites (two camera-spawn sites in
`entity_spawner.rs`, one ring site in `target_indicator.rs`, plus the party camera via
`all_ring_layers()` in `camera.rs`).

**FIXED (re-verified): the party-camera union is no longer a hardcoded literal.**
`camera.rs::all_ring_layers()` derives the full union `(1..=MAX_SPLIT_PLAYERS)` from the constant
itself rather than a hand-written `&[0,1,2,3,4]`, so raising `MAX_SPLIT_PLAYERS` can no longer
leave the party/merged-view camera under-covering the higher reserved layers
`ring_layer_for_player` would then produce. `spawn_party_orbit_camera` calls it directly.

**Remaining fragility points:**

1. **`PrefabDef.player_index`'s modulo collision is now warned, but still not prevented — the
   underlying fragility is real, just no longer silent.** `spawn_players_and_camera`
   (`entity_spawner.rs` ~line 762-780) does check every player's `ring_layer_for_player(player_
   index)` against a `seen_layers` map when `own_viewport_only` is true, and `warn!`s if two
   players collide (out-of-range index like `4`, or a plain duplicate like two players both
   authoring `player_index: 1`). This warn already existed in the same commit that introduced
   `own_viewport_only` (`b17aba8`) — the original note that "nothing validates its range" was
   incomplete even then. But the warn is diagnostic only: if a designer ships with the collision
   anyway, both players still silently end up on the same reserved layer and `own_viewport_only`
   is functionally defeated for that pair (each still sees the other's ring), with no other
   runtime symptom. A 5th `Grid` player (index 4 → layer 1 = P1's) hits the identical collision
   and is not separately warned about beyond the same generic check. This is *not* equivalent to
   `PLAYER_LABEL_COLORS`' own documented modulo collision: that one is cosmetic and
   self-announcing (a duplicate tint), this one silently voids a stated visibility guarantee for
   the colliding pair even with the warning present, unless the designer actually reads scene-load
   logs.
2. `RenderLayers` is inserted on ring **spawn** only, never on resource change. Currently safe only
   because every `TargetRingVisibilityMode` write is paired with a `LevelEntity` teardown that
   despawns all rings. Any future mid-scene toggle (settings menu) needs an `is_changed()` branch.

**How to apply:** when debugging "own_viewport_only doesn't work" / "rings invisible" /
"wrong player's ring in my viewport", check authored `player_index` values first (>= 4 or colliding
mod 4, and check scene-load logs for the collision warn — a designer may have shipped past it),
before suspecting the layer plumbing. Related: [[project_loadscene_teardown_atomicity]] (why
point 2 is currently safe). Gamepad routing had a similar-shaped unvalidated-index fragility
(`resolve_gamepad`'s positional lookup) that has since been closed by `BoundGamepad`/
`gamepad_bind_system`'s `claimed` invariant — see `crates/ironhold_core/src/CLAUDE.md`'s
"Gamepad routing" section, not a standalone memory file anymore.

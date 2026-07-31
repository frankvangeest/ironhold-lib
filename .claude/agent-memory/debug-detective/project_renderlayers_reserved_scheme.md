---
name: renderlayers-reserved-scheme
description: The per-split-player RenderLayers scheme (layers 1-4) has three structural fragility points — modulo collision on out-of-range player_index, a hardcoded party-camera union, and spawn-time-only application
metadata:
  type: project
---

`SplitScreenDef.own_viewport_only` (added 2026-07-31) is the codebase's first designer-facing
`RenderLayers` user. Reserved layers 1-4, computed as `1 + player_index % MAX_SPLIT_PLAYERS` at
four independent sites (two camera-spawn sites in `entity_spawner.rs`, one ring site in
`target_indicator.rs`, plus a hardcoded `&[0,1,2,3,4]` union on the party camera in `camera.rs`).

**Why:** three fragility points that are *not* obvious from reading any single site, and that a
future `RenderLayers` consumer will hit again:
1. `PrefabDef.player_index` is designer-authored with `#[serde(default)]` and **nothing validates
   its range** — the only guard anywhere is the duplicate-`player_index: 0` warn in
   `spawn_players_and_camera`. Two players authored `1` and `5` both map to layer 2, so both
   cameras and both rings share a layer and the opt-in silently becomes a no-op. Same for a
   5th `Grid` player (index 4 → layer 1 = P1's), which leaks into P1's viewport because
   `tab_targeting_system` has no camera dependency — a cameraless capped player still targets.
   This is *not* equivalent to `PLAYER_LABEL_COLORS`' documented modulo collision: that one is
   cosmetic and self-announcing, this one silently voids a correctness guarantee.
2. The party-camera union is a literal, not derived from `MAX_SPLIT_PLAYERS`. Raising that constant
   (which `spawn_players_and_camera`'s own warn! explicitly invites) makes the union under-cover
   and the merged/dynamic view renders *zero* rings — the exact defect plan review caught.
3. `RenderLayers` is inserted on ring **spawn** only, never on resource change. Currently safe only
   because every `TargetRingVisibilityMode` write is paired with a `LevelEntity` teardown that
   despawns all rings. Any future mid-scene toggle (settings menu) needs an `is_changed()` branch.

**How to apply:** when debugging "own_viewport_only doesn't work" / "rings invisible" /
"wrong player's ring in my viewport", check authored `player_index` values first (>= 4 or colliding
mod 4), before suspecting the layer plumbing. When extending the scheme, derive both the per-player
layer and the union from `MAX_SPLIT_PLAYERS` in one shared helper. Related:
[[project_gamepad_index_routing]] (same class of unvalidated index premise),
[[project_loadscene_teardown_atomicity]] (why point 3 is currently safe).

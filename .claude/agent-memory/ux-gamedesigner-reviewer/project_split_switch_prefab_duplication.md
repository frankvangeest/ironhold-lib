---
name: split-switch-prefab-duplication
description: Every split-screen switch lives on the FIRST player's prefab, and scene entities cannot override prefab components — so each variant demo scene requires a full duplicate of the player prefab pair
metadata:
  type: project
---

`SplitScreenDef` (and `PartyZoomDef`) is authored on `components.camera.split` of the **first
player prefab**, and a `GameSceneV2` scene entity has only four fields — `id`, `prefab`,
`transform`, `label` (docs/20_data_formats.md ~line 254). There is **no per-entity component
override**.

Consequence, confirmed 2026-07-29 in `local_coop_demo`: demonstrating a variant of a split-screen
switch in a second scene requires **duplicating the entire player prefab pair**. Adding
`own_viewport_only: true` for `room9` produced `player_p1_split_ring`/`player_p2_split_ring` —
~115 lines that are verbatim copies of `player_p1_split`/`player_p2_split` apart from one bool.
The same pattern already exists for `player_p*_grid`, `player_p*_dynamic`, `player_p*_primitive`.

**Why it matters:** the duplicates immediately drift. `player_p1_split_ring` lost all of
`player_p1_split`'s explanatory comments, and `room9`'s `ActionBar` slots lost room3's
`gamepad_key: "RightTrigger"` — so a controller player in room9 can move but not use the ability.
Nobody notices, because nothing cross-checks near-identical prefabs.

**How to apply:** when a new `SplitScreenDef`/`CameraConfig` toggle is proposed, expect the demo
cost to be a full prefab-pair clone and check the clone for silent drift against its source
(comments, gamepad pairings, stat templates). If a toggle is conceptually *scene*-scoped (ring
visibility, HUD behaviour) rather than *camera*-scoped, argue for putting it on the scene block it
visually belongs to (e.g. `target_indicator:`) — that avoids the clone entirely. Related:
[[local-coop-system]], [[per-player-targeting-gating]].

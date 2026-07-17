---
name: player-index-owner-player-wiring
description: owner_player (ActionBar) matches a player prefab's player_index field; docs terminology gap and recurring "nothing reads player_index yet" stale claims
metadata:
  type: project
---

`ActionBarDef.owner_player: Option<u32>` (Phase 2 of per_player_split_screen_targeting) routes a
bar to a player whose `PrefabDef.player_index` matches. Semantics: `None`/`Some(0)` = primary
player, `Some(n)` = the player prefab with `player_index: n`.

**Terminology gap for designers:** docs describe `owner_player` as "matched against PlayerIndex"
(the component/programmer name) but never tell the designer that the authored field is
`player_index` on the player PrefabDef. The two field docs do not cross-link.

**Recurring stale claim to watch for:** multiple docs/comments assert `player_index` is "not read
by any system yet / reserved for future". Phase 2's `owner_player` action bars now DO read it, so
those lines contradict the shipped feature. Known locations (verify current state before citing —
these may get fixed): docs/20_data_formats.md `player_index` field row; docs/20_data_formats.md
targeting-overview paragraph ("Per-player ability execution is a larger, not-yet-built feature");
local_coop_demo/prefabs/prefabs.ron player prefab comment.

**Why:** these were true pre-Phase-2 and were written when player_index was only forwarded for a
future HUD/nameplate consumer. **How to apply:** on any per-player/split-screen review, grep for
"not yet read"/"not-yet-built"/"reserved for future" around player_index and flag as stale.
Canonical shipped example: local_coop_demo room3.scene.ron (owner_player 0/1, keys G/L).
See [[project_per_player_targeting_gating]] and [[project_action_bar_single_player_assumptions]].

**Second stale-claim class — "primitive/capsule player never gets PlayerIndex":** docs/20_data_
formats.md:~479 defines "primary player" as `PlayerIndex(0)` OR no PlayerIndex, giving "e.g. the
primitive/capsule player path" as the no-PlayerIndex example. The `player_model_source_unification`
feature (planning/features/, v1) makes primitive players get a PlayerIndex like GLB players, so
that parenthetical example goes false. Same claim lives in core CLAUDE.md "four player-construction
sites". Watch for it on any player-spawn-unification review. NOTE: local_coop_demo has ZERO
primitive-bodied players (all `kind: Actor` GLB); the only shipped single-primitive-player is
primitive_world's `player_capsule` prefab (kind: Primitive, Capsule3d) — that's the v1 regression
baseline. PlayerConfig is confirmed Rust-internal (derives Deserialize but no RON references it),
so renaming its fields is genuinely zero designer-facing surface.

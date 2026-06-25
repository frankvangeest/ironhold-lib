---
name: nameplate-system
description: Nameplate system (NameplateOptionsDef/NameplateBarDef, show_nameplates, display_name, nameplate override); stat-scope footgun makes player + mana bars show nothing in canonical example
metadata:
  type: project
---

Nameplate system (added ~2026-06). Floating name + pixel stat-bar widget above 3D entities, fully RON-driven. Canonical example: `3rd_person_game_demo`.

Schema/doc locations:
- `docs/20_data_formats.md`: GameSceneV2 rows `show_nameplates`/`nameplate_options` (~180); "Nameplate system (NameplateOptionsDef)" section (~444) with NameplateOptionsDef table, NameplateBarDef table, NameplateFactionFilter variants, per-prefab override note, full RON example; PrefabDef rows `display_name`/`nameplate` (~1424).
- Scene config: `scenes/main.scene.ron` `show_nameplates: true` + `nameplate_options` block. Prefabs: `display_name` on all named prefabs; `nameplate: true` on the 3 player prefabs only.

Key behaviors: `show_nameplates` scene gate (default false); `faction_filter` HostileOnly(default)/FriendlyOnly/All; per-prefab `nameplate: true/false/absent` 3-state override (true bypasses faction filter, still respects max_distance; false suppresses entirely; absent inherits). `display_name` falls back to prefab key. Stat bars use `{self}.health` substitution; bars referencing an absent stat are SILENTLY skipped.

**Critical footgun in the shipped example (the dominant designer trap here):**
The scene `stat_bars` configure two bars: `{self}.health` and `{self}.mana`. But stat SCOPING differs by entity type:
- Enemies (zombie/snake/spider) define a per-entity `stat_templates` with key `health` only — NO `mana` template. So `{self}.mana` is silently skipped on every enemy.
- Player prefabs (player_male/female/warrior) have `nameplate: true` but NO `stat_templates` at all. The player uses GLOBAL stat keys `player_health`/`player_mana` (from stats/stats.ron), not entity-scoped `player_01.health`. So `{self}.health` AND `{self}.mana` both resolve to non-existent `player_01.*` and are silently skipped — the forced player nameplate shows NAME ONLY, zero bars.
Net result: the configured mana bar appears on NO entity, and the player's forced nameplate has no bars. Looks broken/half-finished to a designer copying it, with no error feedback.

**Why this matters:** the global-vs-entity-scoped stat split (player on `player_health`, enemies on `{self}.health` templates) is the same conceptual trap as the inventory player-vs-spawn-id magic string — see [[inventory-item-system]]. Nameplate `{self}.stat` only works on entities that have a `stat_templates` entry for that stat. Docs DO warn bars are silently skipped, but never explain WHY the player specifically gets no bars, nor that `{self}` requires a per-entity stat template.

Color tuples here are correctly 4-tuple RGBA throughout (name_color, fill_color, bg_color) — consistent, no arity drift. See [[color-tuple-inconsistency]].

Doc example `chest_01`/`player_warrior` blocks use `...` ellipsis and show a `nameplate: false` chest that does NOT exist in the shipped prefabs (shipped chest_01 has no nameplate field) — illustrative-only, fine, but no SHIPPED example of `nameplate: false` exists to copy.

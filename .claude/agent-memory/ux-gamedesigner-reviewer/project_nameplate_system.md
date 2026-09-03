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

**v1 player-visibility toggle (added ~2026-07):** `NameplateOptionsDef.show_player_nameplate: bool` (default `false`) gates the PLAYER's own nameplate, orthogonal to `show_nameplates` (which now governs NPCs/props only). `faction_filter` NEVER applies to the player. Per-prefab `nameplate: Some(true/false)` still wins over `show_player_nameplate`, same as for `show_nameplates`. Docs (20_data_formats ~180/~444/~1497) + schema comments (scene_v2.rs, catalog.rs) are internally consistent and cross-referenced. Genre-convention default (`false`) is documented in field desc + section prose + RON example inline comment. NOT called out as a migration WARNING for new authors who set `show_nameplates: true` expecting player coverage — the friction lands mainly because `show_player_nameplate` sits BELOW `show_nameplates` in the field table so it's easy to miss.

**v2 runtime toggle `Action::ToggleOwnNameplate` (reviewed ~2026-07):** no-args tuple action; flips a `PlayerNameplatePreference` resource; emits `nameplate.own_shown`/`nameplate.own_hidden` (mirrors ToggleMute's audio.muted/unmuted). Bind pattern = IconButton `bind` GameVariable + a `SetVariable` bridge rule, IDENTICAL to canonical `hud_audio_toggle` (docs point at it rather than duplicating — pointer is clear enough, no separate example needed). Docs: 20_data_formats actions row (~2219), "Runtime player toggle" callout blockquote (~454), assets/projects/CLAUDE.md tuple-variant list (~59), schema comment actions.rs (~71). Precedence: per-prefab `nameplate: Some(...)` override always wins (toggle still flips resource but visibility never changes — a no-op the designer docs DON'T explain, so a "button does nothing" case with a prior override is undiagnosable). NOT persistent — resets to `show_player_nameplate` on every LoadScene; this is the top designer trap (looks like a bug) and is currently only calm prose in the callout, NOT a warning-style callout. Also: `nameplate.own_shown/own_hidden` events are NOT in the events reference table (unlike audio.muted which is), and the callout doesn't restate that the SetVariable bridge rule is required.

**v3 zoom/depth scaling (feature/nameplate-zoom-spacing, ~2026-08):** nameplates now inherit the scene's `label_depth_scale` (previously hardcoded off — fixed screen size at every zoom). No per-widget override. `3rd_person_game_demo/scenes/main.scene.ron` gained `label_depth_scale: (reference_distance: 8.0, min_scale: 0.5)` tuned to its Orbit `min_radius 3.0`/`max_radius 18.0`. **CLOSED: `local_coop_demo` room3/room9/room10 now all set their own `label_depth_scale`** — no longer unfixed copy-from examples. Still verify the docs' own nameplate RON example block (20_data_formats ~607) includes `label_depth_scale` before citing it as a safe copy source. See [[depth-scale-field-scope]] for the formula and its clamp-at-1.0 limitation.

**v3.1 round 2 (2026-08-16):** demo retuned to `(12.0, 0.5)`; new `screen_offset` field on
`StatLabelDef`/`WorldStatBarDef` for pixel-stacking a nameplate + number + bar — see
[[screen-offset-stacking]] for the shared-`offset` rule, the mismatched defaults, and the snake
exception. `nameplate_options.offset` remains scene-wide with NO per-prefab override, which is the
root cause of the snake's nameplate floating above its body and of the cross-file coupling the new
pattern requires. The demo's "Toggle Nameplate" HUD button was renamed to "Nameplate" (16 chars
overflowed a 200px button at the engine's fixed 26px button font — `ButtonDef` has no `font_size`).

Color tuples here are correctly 4-tuple RGBA throughout (name_color, fill_color, bg_color) — consistent, no arity drift.

Doc example `chest_01`/`player_warrior` blocks use `...` ellipsis and show a `nameplate: false` chest that does NOT exist in the shipped prefabs (shipped chest_01 has no nameplate field) — illustrative-only, fine, but no SHIPPED example of `nameplate: false` exists to copy.

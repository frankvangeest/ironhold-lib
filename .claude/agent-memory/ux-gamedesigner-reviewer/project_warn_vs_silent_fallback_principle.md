---
name: warn-vs-silent-fallback-principle
description: When Ironhold chooses to warn! vs silently fall back on a designer authoring omission — the discriminating principle and the precedents on each side
metadata:
  type: project
---

Ironhold's de-facto rule for whether a designer authoring gap produces a `warn!` (and often a paired `ironhold_cli validate` error) or a silent fallback:

**Warn when the designer's authored intent is ambiguous or self-contradictory** (config that can't do what it evidently sets out to do). **Stay silent when the fallback is a legitimate, common authoring choice.**

Precedents that WARN (intent ambiguous/contradictory):
- `warn_cross_bar_duplicate_keys` (scene_loader.rs) — two bars share a slot key; also a cli `cross_bar_duplicate_key` error. Fires at SCENE LOAD, not per-frame.
- Duplicate `player_index` across players (entity_spawner.rs ~line 534) — two "primary" players.
- Missing `animation_policy` on a player prefab (`assemble_player_config`, entity_spawner.rs:939-942) — a player-authoring omission that plainly wasn't intended.
- Camera: `party`+`split` both set (split wins + warn); neither set (falls back to single OrbitCamera + warn).
- `stat_overrides` unknown key (docs 20_data_formats.md:254) — warns.

Precedents that stay SILENT (legitimate common choice):
- Nameplate/`world_stat_bar` stat bar for a stat the entity lacks — "silently skipped, no error" (docs:642). A skeleton with no mana bar is normal.
- Player using global `player_health` silently failing to match `{self}.*` (docs:618-624) — documented, not warned.

**How to apply:** When reviewing a new fallback path, recommend WARN if the fallback contradicts an evident per-entity/per-player intent signal (e.g. `owner_player` set, 2+ players, a partial `stat_templates` block present but missing the referenced key). Recommend SILENT if the fallback is the ordinary single-player/global case. Always specify TIMING: prefer scene-load or cli-validate time (follows `warn_cross_bar_duplicate_keys`) over per-frame/per-press, which spams the log.

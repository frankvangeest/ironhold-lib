---
name: per-player-stat-pools-pattern
description: Per-player action-bar cost pools via PlayerConfig.stat_templates → StatMap on player entity; SlotCost resolves own-pool-first/global-fallback; the player spawn path does NOT forward stat_label/world_stat_bar (silent-drop footgun)
metadata:
  type: project
---

Reviewed 2026-07-17 (`feature/per-player-stat-pools`). Gives split-screen players their own
StatMap-backed pool for action-bar `SlotCost`, reusing `PrefabDef.stat_templates` (the field NPCs
already use). Builds on [[per-player-action-bar-pattern]] and [[stat-overrides-flow]].

**Core design is fully RON-reachable and correctly data-driven (ALIGNED):**
- `PlayerConfig.stat_templates: Vec<StatTemplateDef>` (`schema/player.rs`, `#[serde(default)]`),
  forwarded from `prefab.stat_templates.clone()` in `assemble_player_config` (entity_spawner.rs
  ~996 — the single source of truth for the 2 player-assembly sites). `spawn_player_entity_core`
  inserts the built `StatMap` when non-empty (entity_spawner.rs ~838). Omit the block → no StatMap
  → `SlotCost` falls back to global `LoadedStats` exactly as pre-feature. Backward-compat is the
  load-bearing property.
- `build_stat_map_from_templates` (entity_spawner.rs ~109) extracted from `attach_prefab_features`
  = shared StatMap-build for both generic-prefab and player paths (kills a divergence risk noted in
  [[stat-overrides-flow]]).
- `resolve_cost_source(stat, spawn_id, player_stats, loaded_stats)` (action_bar.rs ~238): own
  StatMap first → returns dot-routed `"{spawn_id}.{stat}"`; else global → returns `None` (undotted
  key). Resolved ONCE per firing slot in `action_bar_input_system` and reused for BOTH the gate
  check and the deduct action's key (system-architect correctness requirement — the check is a sync
  read, the deduct is a deferred `Action::ModifyStat`; must not each re-decide which pool).
  `action_bar_visual_system` re-resolves independently for dimming (read-only, fine).
- No executor change: `Action::ModifyStat`/`SetStat` already `split_once('.')` → look up entity by
  SpawnId → its StatMap (action_executor.rs ~397/425). Player has SpawnId + StatMap, so
  `"player_01.mana"` routes to the player's own pool. No hardcoded stat name, no hardcoded player
  count — the dot-route format is a generic mechanism.
- No ActionQueue anti-pattern: deduct is added to `PendingIntentActions`, flushed by
  `flush_pending_intent_system`; so a `rules.ron` intercept suppresses the deduct too (documented).

**Warning/CLI validate correctly avoids single-player false positives:** both
`warn_missing_player_stat_templates` (scene_loader.rs ~1467) and validate.rs'
`missing_player_stat_template` check require `bar.owner_player` set AND the matched player's
`stat_templates` non-empty AND the cost stat absent from them. A single-player bar (owner_player
omitted) or a player with no stat_templates is skipped → no spurious warnings on existing projects.

**FOOTGUN NOW FIXED (2026-07-17, `feature/player-stat-widgets` — see [[player-stat-widgets-pattern]]).**
`PlayerConfig` gained `stat_label`/`world_stat_bar` (forwarded in `assemble_player_config`);
`spawn_player_entity_core` + the primitive inline path + `drain_spawn_queue_system` all push a
`{self}`-resolved `DynamicStatUiEntry`, so a player prefab's floating stat widget now renders
exactly like any NPC/prop. The description below is the PRE-FIX state, kept for history:

**FOOTGUN (pre-2026-07-17): the player spawn path silently dropped `stat_label`
and `world_stat_bar`.** Players branch at scene_loader.rs ~625 (`is_player` →
`assemble_player_config`) BEFORE the generic path's stat_label/world_stat_bar push (~657/~592/~387).
`PlayerConfig` has NO stat_label/world_stat_bar field, so those blocks authored on a player prefab
are silently ignored — and HUD `Label bind:` (DynamicLabel, lib.rs ~328) resolves only
`GameVariables`, NOT `resolve_stat`/StatMap, so there is NO RON route to display a player's own
StatMap value as a floating label. Only `stat_label`/`world_stat_bar`/`stat_radar` use `resolve_stat`
(dot-routed StatMap-aware), and those are exactly what the player path doesn't forward. The
per-player-stat-pools demo (local_coop_demo player_p1_split/p2_split) authored `stat_label:
(stat_key: "{self}.mana")` on the players expecting a visual confirmation — it will NOT render.
Working per-player confirmation that DOES survive: the action-bar dim (`cost_ok` in
action_bar_visual_system). CLI validate/asset_checker do NOT catch this (valid schema, ignored at
runtime → false confidence). Fix options: add stat_label/world_stat_bar forwarding to
PlayerConfig + spawn_player_entity_core, OR change the demo to a working confirmation route.

**Minor:** player path passes `&HashMap::new()` for stat_overrides, so `SceneEntityDef.stat_overrides`
can't tune a player's pool per-instance (asymmetry with NPCs; near-irrelevant since players are 1
instance/prefab). And the missing-template warning only covers owner_player=Some bars, while runtime
cost resolution ALSO applies to owner_player=None bars against the primary player's StatMap — a
mismatched-key None-bar falls back to global silently with no warning.

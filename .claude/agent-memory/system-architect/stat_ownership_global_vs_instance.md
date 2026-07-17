---
name: stat-ownership-global-vs-instance
description: Global LoadedStats vs per-entity StatMap; how SlotCost/action-bar costs route; how to make costs per-player without new schema
metadata:
  type: project
---

Two parallel stat stores exist and the split is load-bearing:
- **Global** — `LoadedStats(HashMap<String,LiveStat>)` resource (`schema/stats.rs`), authored in `stats.ron`, persists across scenes. Addressed by a plain key (`"mana"`).
- **Per-entity** — `StatMap(IndexMap<String,LiveStat>)` **component** (`schema/stats.rs`, IndexMap for deterministic replay iteration), built from `PrefabDef.stat_templates` (`catalog.rs:~794`) by `attach_prefab_features` (`entity_spawner.rs:~81-114`). Addressed by dot key `"{spawn_id}.{stat}"`.

**All four `capabilities/stats.rs` tick systems (modifier/effective/regen/threshold) iterate `Query<&mut StatMap>` unconditionally** — any entity that gains a StatMap is ticked identically to NPCs, zero extra code. `ModifyStat`/`SetStat` in `action_executor.rs:~397-452` already `split_once('.')` to route dot keys to the entity's StatMap via the spawn registry.

**Per-player stat pools shipped on feature/per-player-stat-pools (2026-07-17), exactly as the plan predicted:**
- Players CAN now carry a StatMap. `PlayerConfig.stat_templates` (constructed-in-code by `assemble_player_config`, NOT deserialized from scene RON — `#[serde(default)]` field, zero RON-schema/migration impact) forwards `prefab.stat_templates`; `spawn_player_entity_core` inserts the StatMap when non-empty. The stat-template→StatMap conversion was factored into shared helper `build_stat_map_from_templates` (`entity_spawner.rs`), now called by BOTH `attach_prefab_features` (NPC/prop path) and `spawn_player_entity_core` (player path). Site 2 (primitive/capsule player, `scene_loader.rs`) still gets no StatMap — single-player-only, intentionally untouched.
- `SlotCost` is no longer global-only. `resolve_cost_source(stat, player_stats, loaded_stats) -> (f32, bool)` (`action_bar.rs`) resolves per-player-first / global-fallback keyed **per-stat** (`player StatMap.get(stat)`, not StatMap presence). Returns `(current, use_player_pool)` — deliberately does NOT build the dot key (no `format!` alloc; visual system calls it every frame). Input system builds `"{spawn_id}.{stat}"` for the deduct only inside the `just_pressed` loop.
- Correctness invariant satisfied: `action_bar_input_system` resolves ONCE per firing slot and reuses the `(current, use_player_pool)` tuple for both the gate check and the deduct-action key, so check and deduct can't disagree on which pool. (visual system's separate resolution is read-only display — fine.)
- Load-time `warn_missing_player_stat_templates` (`scene_loader.rs`) + CLI `missing_player_stat_template` (`validate.rs`) fire ONLY when the owning player declares SOME `stat_templates` but not the cost's stat — silent on the no-`stat_templates` global-fallback path (single-player unaffected). Both scoped by `owner_player` == `player_index`.
- Backward compat is the load-bearing property: a player prefab with no `stat_templates` gets no StatMap → SlotCost falls through to global `LoadedStats` byte-for-byte as before.

**Correctness footgun for any FUTURE per-player-cost change** (still applies): the check is a synchronous direct read but the deduct is a deferred `Action::ModifyStat` (pending → ActionQueue → executor). Keep computing the "own pool vs global" boolean ONCE. Also: a `rules.ron` rule that overrides a slot's intent suppresses the ENTIRE pending entry incl. the cost deduct (`flush_pending_intent_system`), so an overridden slot drains no pool — applies per-player too. Known gap (logged in claude_suggestions.md): `stat_label`/`world_stat_bar` blocks are only wired for the NPC/prop spawn path, so they're silently dropped on player prefabs.

See [[player_spawn_paths]] for the four player-construction sites.

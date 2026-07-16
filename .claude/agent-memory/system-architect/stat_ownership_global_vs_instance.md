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

**Player entities are the only spawn path that never gets a StatMap** (as of 2026-07-17). GLB players go through `spawn_player_entity_core` (`entity_spawner.rs:~715`); primitive/capsule players through `scene_loader.rs:~704-830` (site 2, single-player-only, never calls `attach_prefab_features`). Neither attaches a StatMap.

**Action-bar `SlotCost` (`action_bar.rs`) is global-only today**: check (`:~160-172`) + dim (`:~268-275`) read `LoadedStats` directly; deduct (`:~199-205`) pushes a plain-key `Action::ModifyStat` → routes to global. In split-screen every player's bar silently shares one global pool. This is the documented "Per-player stat/resource pools" gap.

**To make costs per-player with ZERO new RON schema** (per_player_stat_pools plan, verified sound): forward `prefab.stat_templates` onto `PlayerConfig` (constructed-in-code by `assemble_player_config` `:~931`, NOT deserialized from scene RON — adding a `#[serde(default)]` field has no RON-schema/migration impact), insert the StatMap on the player in `spawn_player_entity_core`, then make SlotCost resolve per-player-first / global-fallback keyed **per-stat** (`stat_map.contains_key(cost.stat)`, not StatMap presence).

**Correctness footgun for any per-player-cost change**: the check is a synchronous direct read but the deduct is a deferred `Action::ModifyStat` (pending → ActionQueue → executor). Compute the "own pool vs global" boolean ONCE and drive both the gate read and the deduct-action key from it, or they can diverge (check passes, wrong pool drained). Also: a `rules.ron` rule that overrides a slot's intent suppresses the ENTIRE pending entry incl. the cost deduct (`flush_pending_intent_system`), so an overridden slot drains no pool — existing behavior, applies per-player too.

See [[player_spawn_paths]] for the four player-construction sites.

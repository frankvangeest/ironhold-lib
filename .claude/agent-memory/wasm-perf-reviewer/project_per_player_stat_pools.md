---
name: per-player-stat-pools
description: per-player stat pools — resolve_cost_source allocates a throwaway format! String per cost-slot per-frame in action_bar_visual_system (per-player-pool slots only)
metadata:
  type: project
---

Per-player stat pools (feature/per-player-stat-pools, reviewed 2026-07-17). Touches `capabilities/action_bar.rs`, `capabilities/stats.rs`, `runtime/scene_manager/entity_spawner.rs`. Zero new deps, no WASM-incompatible API (format!/HashMap/String only).

**resolve_cost_source (action_bar.rs ~L238)** returns `(f32, Option<String>)` where the `Option<String>` is the dot-routed per-player deduct key `format!("{spawn_id}.{stat}")`, built only when the acting player's own `StatMap` contains the stat (per-player pool hit); `None` on global `LoadedStats` fallback.

**The per-frame allocation to watch:** `action_bar_visual_system` (runs every frame, gated `run_if(any_action_slots)`) calls `resolve_cost_source(...).0 >= c.amount` per cost-slot and DISCARDS the `.1` key String. So for every action-bar slot whose cost draws from a per-player pool, it allocates and immediately drops a short `format!` String every frame. Bounded (single-digit slots × short strings) but avoidable, and it violates this codebase's established "per-frame system, zero idle allocation" convention for the action_bar systems (see [[intent-event-layer]]). Clean fix: change `resolve_cost_source` to return `(f32, bool)` (bool = per-player-pool used) preserving single-source-of-truth for the pool decision, and build the `format!` key only at the input_system deduct call site (which is already input-gated). Recommended before commit or log as a claude_suggestion.

**Not concerns:** `action_bar_visual_system`'s new `players.iter().find(owns_slot(...))` per cost-slot is negligible — ≤4 players (MAX_SPLIT_PLAYERS), Copy `PlayerIndex(u32)` predicate, no alloc. `action_bar_input_system`'s added `Option<&StatMap>` query column + cost resolution are all inside the `just_pressed` fired-slot branch (input cadence, not per-frame); archetype state cached so the extra query column is ~free on idle. `build_stat_map_from_templates` is spawn-time only.

**stats.rs tick systems (pre-existing, not introduced here):** `stat_modifier_system` and `stat_threshold_system` each do `let keys: Vec<String> = stat_map.0.keys().cloned().collect()` PER StatMap entity PER frame. Adding StatMap to 2-4 player entities adds that many more key-clone Vecs/frame to each system — marginal next to NPC StatMap carriers in a populated scene, but in a low-NPC scene the players can become the dominant StatMap carriers. `stat_effective_value_system`/`stat_regen_system` iterate values in place (no such alloc). Latent pre-existing pattern worth a separate cleanup, not a regression from this feature.

Related: [[intent-event-layer]], [[project_per_player_targeting]].

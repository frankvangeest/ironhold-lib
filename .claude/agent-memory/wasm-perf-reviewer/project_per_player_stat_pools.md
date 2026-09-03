---
name: per-player-stat-pools
description: per-player stat pools — resolve_cost_source allocates a throwaway format! String per cost-slot per-frame in action_bar_visual_system (per-player-pool slots only)
metadata:
  type: project
---

Per-player stat pools (feature/per-player-stat-pools, reviewed 2026-07-17). Touches `capabilities/action_bar.rs`, `capabilities/stats.rs`, `runtime/scene_manager/entity_spawner.rs`. Zero new deps, no WASM-incompatible API (format!/HashMap/String only).

**resolve_cost_source (action_bar.rs ~L287)** returns `(f32, bool)` — the bool flags whether the
value came from the acting player's own per-player `StatMap` (pool hit) vs. the global
`LoadedStats` fallback. **Fixed:** this used to return `(f32, Option<String>)`, building a
throwaway `format!("{spawn_id}.{stat}")` deduct-key String per cost-slot per-frame in
`action_bar_visual_system` even though that system only ever read `.0` and discarded the key. The
`(f32, bool)` signature closes that — no `format!` call happens in the visual system's per-frame
path at all now; the deduct key (if needed) is built only at the input_system call site, which is
already input-gated. Verified current (2026-09-03): `resolve_cost_source` signature at
`capabilities/action_bar.rs` ~L287-291 matches this exactly.

**Not concerns:** `action_bar_visual_system`'s new `players.iter().find(owns_slot(...))` per cost-slot is negligible — ≤4 players (MAX_SPLIT_PLAYERS), Copy `PlayerIndex(u32)` predicate, no alloc. `action_bar_input_system`'s added `Option<&StatMap>` query column + cost resolution are all inside the `just_pressed` fired-slot branch (input cadence, not per-frame); archetype state cached so the extra query column is ~free on idle. `build_stat_map_from_templates` is spawn-time only.

**stats.rs tick systems (pre-existing, not introduced here):** `stat_modifier_system` and `stat_threshold_system` each do `let keys: Vec<String> = stat_map.0.keys().cloned().collect()` PER StatMap entity PER frame. Adding StatMap to 2-4 player entities adds that many more key-clone Vecs/frame to each system — marginal next to NPC StatMap carriers in a populated scene, but in a low-NPC scene the players can become the dominant StatMap carriers. `stat_effective_value_system`/`stat_regen_system` iterate values in place (no such alloc). Latent pre-existing pattern worth a separate cleanup, not a regression from this feature.

Related: [[intent-event-layer]], [[project_per_player_targeting]].

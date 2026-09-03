---
name: nameplate-gating
description: Nameplate show/suppress gating — should_insert_nameplate single source of truth, the character-select spawn bypass bug, and faction_filter's player-lumping flaw
metadata:
  type: project
---

Nameplate display gating in ironhold_core, as shipped (the 2026-07-03 "Recommended direction" below was implemented in full):

1. `should_insert_nameplate(nameplate: Option<bool>, show: bool) -> bool` in `runtime/scene_manager/mod.rs` (beside `resolve_label_depth_scale`) is the single source of truth for spawn-time gating. Tri-state: `Some(false)` always suppress, `Some(true)` always show, `None` inherit `show`. It is now used at **all 6** spawn sites (scene_loader.rs ×5, entity_spawner.rs ×1) — the character-select dynamic player-spawn path in `action_executor.rs` was fixed to route through it too, using the player-specific `show` source described below. Per `crates/ironhold_core/src/CLAUDE.md`, this is documented as the single source of truth for all 6 call sites.

2. **`show` differs by entity type, resolved BEFORE calling `should_insert_nameplate`, not by a second gating function.** NPCs/props use `scene.show_nameplates`/`nameplate_config.enabled`; `Player`-tagged entities use the independent `show_player_nameplate` / `nameplate_config.player_enabled` instead — exactly the orthogonal `Option<bool>` field the 2026-07-03 review recommended (on `NameplateOptionsDef`, zero migration, `3rd_person_game_demo` untouched). This closes the old character-select bypass bug.

3. `NameplateFactionFilter` (HostileOnly/FriendlyOnly/All) in `nameplate_visibility_system` is now documented as **NPC/prop-only** — `Player` entities bypass `faction_filter` entirely (same treatment as a `Some(true)` override: distance-only), since faction hostility doesn't apply to "should I see my own name." This resolves the old "misclassifies the player as friendly" concern by routing players around the filter rather than fixing the filter's `!NpcAgent` predicate itself — see [[player-marker-gap]] for the (separate, still-relevant) general "no dedicated Player marker" history; a real `Player` marker now exists (`capabilities/player.rs`) and is what this bypass uses.

4. **`Action::ToggleOwnNameplate` shipped** (the deferred player-preference follow-up) — flips a `PlayerNameplatePreference` resource, consumed only by `nameplate_visibility_system`'s per-frame `Player`-entity branch (spawn-time `NameplateTag` insertion is untouched by a toggle). An explicit per-prefab `nameplate: Some(true)`/`Some(false)` always wins over this preference. Re-seeded from `show_player_nameplate` on every scene load — does not persist across scene transitions (deliberate simplicity choice).

**How to apply:** Visibility is render-only (no determinism/sim-state impact). This design is now the reference pattern for any future scene-wide-vs-player-preference visibility split (e.g. an analogous `show_own_*` toggle for another per-player HUD element).

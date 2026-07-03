---
name: nameplate-gating
description: Nameplate show/suppress gating — should_insert_nameplate single source of truth, the character-select spawn bypass bug, and faction_filter's player-lumping flaw
metadata:
  type: project
---

Nameplate display gating in ironhold_core has three moving parts:

1. `should_insert_nameplate(nameplate: Option<bool>, show: bool) -> bool` in `runtime/scene_manager/mod.rs` (~324) is the intended single source of truth for spawn-time gating. Tri-state: `Some(false)` always suppress, `Some(true)` always show, `None` inherit `show`. Used at 5 of 6 spawn sites (scene_loader.rs ×4, entity_spawner.rs ×1).

2. **The 6th site bypasses it (known bug):** the character-select dynamic player-spawn path in `action_executor.rs` (~163, `Action::Spawn` for a player-tagged prefab) uses a truncated `prefab_def.nameplate != Some(false)` predicate that ignores `show_nameplates` entirely. Tracked in planning/backlog.md Bugs. Root product question: should the player's own nameplate honor the scene setting, or always show unless explicitly disabled?

3. `NameplateFactionFilter` (HostileOnly/FriendlyOnly/All) in `nameplate_visibility_system` (capabilities/nameplate.rs ~184) implements FriendlyOnly as `!NpcAgent` — see [[player-marker-gap]]. This misclassifies the player and decorative non-NPCs as "friendly."

**Recommended direction (2026-07-03 review):** Do NOT split `show_nameplates: bool` into parallel `show_npc_nameplates` + `show_player_nameplates` top-level fields — `show_nameplates` already IS the NPC/world switch. Instead add an orthogonal `show_player_nameplate: Option<bool>` (default None = inherit) to `NameplateOptionsDef` — zero migration, keeps 3rd_person_game_demo untouched — and route the executor player path through `should_insert_nameplate` with the resolved player flag. Keep `faction_filter`; fix it to use a real Player marker rather than retire it. A deferred `ToggleOwnNameplate` runtime action (mirrors ToggleMute/SetVolume) is the player-preference follow-up.

**How to apply:** Visibility is render-only (no determinism/sim-state impact). Any change here crosses schema + a new ECS marker + 6 capabilities → warrants a planning/features/ doc per project rules.

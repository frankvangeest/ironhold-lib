---
name: nameplate-system-pattern
description: Nameplate system touchpoints — scene-level show_nameplates + nameplate_options, per-prefab display_name/nameplate override, NameplateTag on all 6 spawn paths, reuses WorldPixelBarFillMarker
metadata:
  type: project
---

The nameplate system (`capabilities/nameplate.rs`) is a scene-managed name + pixel-bar widget
above 3D entities, distinct from `stat_label`/`world_stat_bar` (always-visible per-prefab). It is
the cleanest data-driven example so far of a NON-action cosmetic capability.

**Designer entry points:**
- `GameSceneV2.show_nameplates: bool` + `nameplate_options: Option<NameplateOptionsDef>`
  (schema/scene_v2.rs ~line 98). `NameplateOptionsDef` holds faction_filter
  (`NameplateFactionFilter` enum HostileOnly/FriendlyOnly/All), max_distance, offset,
  name_font_size, name_color, text_shadow, stat_bars (`Vec<NameplateBarDef>` — stat_key with
  `{self}` substitution + fill_color/bg_color), bar_width/height/spacing. All fields `#[serde(default)]`.
- `PrefabDef.display_name: Option<String>` + `nameplate: Option<bool>` (schema/catalog.rs ~855).
  None=inherit scene; Some(true)=always show (bypass faction, respect distance); Some(false)=never.

**Player nameplate is now a SEPARATE toggle (player_nameplate_visibility v1, shipped 2026-07-03):**
`NameplateOptionsDef.show_player_nameplate: bool` (`#[serde(default)]`=false, schema/scene_v2.rs
~1186) governs the PLAYER's own nameplate INDEPENDENTLY of `show_nameplates` (which now governs
NPCs/props only). Flows to `NameplateSceneConfig.player_enabled` (parallel to `.enabled`) at scene
load (scene_loader.rs ~1166). A real `Player` marker + `PlayerOwnership::{Local,Remote}` enum now
exists (capabilities/player.rs) — purely internal ECS signal, NO RON surface (correct: designer
never sets ownership; always Local until multiplayer). Inserted unconditionally at BOTH player
spawn paths right after tag_spawned_entity: entity_spawner.rs spawn_player_entity ~577 (GLB) and
scene_loader.rs ~780 (primitive). The known character-select bug IS NOW FIXED — action_executor.rs
~164 uses `should_insert_nameplate(prefab_def.nameplate, spawn_params.nameplate_config.player_enabled)`.

**should_insert_nameplate helper (scene_manager/mod.rs ~324) unchanged**:
`nameplate != Some(false) && (show || nameplate == Some(true))`. What changed is the `show` ARG the
player call sites pass: player paths pass `show_player_nameplate`/`player_enabled`, NPC/prop paths
pass `scene.show_nameplates`/`nameplate_config.enabled`.

**Three player-nameplate call sites (all pass show_player_nameplate — verified 2026-07-03):**
1. scene_loader.rs ~635 — scene-placed GLB player (PlayerConfig.nameplate_display_name)
2. scene_loader.rs ~784 — scene-placed primitive player (np_override/np_display_name locals)
3. action_executor.rs ~164 — dynamic character-select player (PlayerConfig via SpawnParams)
`show_player_nameplate` local extracted once at scene_loader.rs ~83 (`.unwrap_or(false)` when
nameplate_options absent). SpawnParams (scene_manager/mod.rs, bundled SystemParam to dodge the
16-param limit) gained `nameplate_config: Res<NameplateSceneConfig>` to reach it at :164.

**NPC/prop spawn paths (pass scene.show_nameplates / nameplate_config.enabled):**
- scene_loader.rs ~390, ~595, ~675 (composite/single-mesh/GLB non-player)
- entity_spawner.rs ~375 — dynamic Action::Spawn NON-player (else-branch after player early-continue);
  reads `nameplate_config.enabled` from RESOURCE (correct; same as stat-bar dynamic path)

**Two per-entity gates in the systems (both NECESSARY consequences of the split, NOT scope creep):**
- `nameplate_setup_system` (Added<NameplateTag>) queries `Option<&Player>` and picks
  `config.player_enabled` vs `config.enabled` per-entity (~line 76). Without it, its own redundant
  `!scene_enabled && prefab_override != Some(true)` gate would re-suppress a legitimately-tagged
  player when show_nameplates=false. Per-prefab `nameplate: Some(true)` still survives (verified).
- `nameplate_visibility_system` (per-frame) has `player_q: Query<(), With<Player>>` and treats
  `player_q.contains(entity)` like `Some(true)` override — distance-only, bypasses faction_filter
  (~line 207). Without it, default HostileOnly would force-hide the player every frame. Correctly
  reasoned: faction hostility is meaningless for "should I see my own name."

`PlayerConfig` (schema/player.rs) is a runtime assembly struct, NOT RON-authored. Its
nameplate_display_name/nameplate_override fields are populated from PrefabDef at scene_loader.rs
~761. The "is this designer-reachable" answer is the PrefabDef fields, not PlayerConfig.

**Correct pipeline behavior:** nameplate_setup_system + nameplate_visibility_system are pure
cosmetic side-effects — NO ActionQueue push, NO message emission, NO asset paths. This is the
correct exception (visibility toggles / cosmetic, like target_indicator.rs). Bars REUSE
`WorldPixelBarFillMarker` + `world_pixel_bar_update_system` from stat_display.rs — no new bar
update system. Anchor is a `WorldLabel` (reuses world_label_screen_pos_system). visibility_system
must run `.after(world_label_screen_pos_system)` (registered correctly in lib.rs ~256).

**Config reset:** `insert_resource(NameplateSceneConfig{...})` runs UNCONDITIONALLY on every scene
load (scene_loader.rs ~1285), so a scene with show_nameplates:false correctly resets prior state.

**Faction filter is a documented v1 stub:** HostileOnly == `has NpcAgent`. Replace when Group
system ships. Acceptable as-is.

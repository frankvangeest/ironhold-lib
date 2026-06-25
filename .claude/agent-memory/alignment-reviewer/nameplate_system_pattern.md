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

**Six spawn paths all insert `NameplateTag` (verified at 4c47cc6+):**
The standard tag-condition is `prefab.nameplate != Some(false) && (scene.show_nameplates || prefab.nameplate == Some(true))`.
1. scene_loader.rs ~451 — composite primitive
2. scene_loader.rs ~721 — single-mesh primitive (or GLB non-player; grep to confirm which)
3. scene_loader.rs ~801 — GLB non-player actor/prop
4. scene_loader.rs ~906 — primitive player (uses np_override/np_display_name locals)
5. entity_spawner.rs ~340 — dynamic Action::Spawn (drain_spawn_queue_system) — reads
   `nameplate_config.enabled` from the RESOURCE not scene context (correct; same as stat-bar dynamic path)
6. entity_spawner.rs ~541 — GLB player via PlayerConfig.nameplate_display_name/nameplate_override

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

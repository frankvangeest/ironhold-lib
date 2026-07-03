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
The tag-condition is now the extracted helper `should_insert_nameplate(nameplate, show)` in
`scene_manager/mod.rs` (beside `tag_spawned_entity`): `nameplate != Some(false) && (show || nameplate == Some(true))`.
5 of the 6 call sites route through it (scene_loader.rs ×4, entity_spawner.rs ×1). The 6th —
`action_executor.rs:163` (dynamic character-select PLAYER spawn) — deliberately does NOT use the
helper: it uses a truncated `nameplate != Some(false)` that ignores `show_nameplates` entirely.
That divergence is a KNOWN BUG tracked in planning/backlog.md ## Bugs ("Character-select player
nameplate ignores show_nameplates"), NOT a candidate for silent inclusion in a refactor — leaving
it out preserves current behavior pending Frank's decision. Note action_executor.rs:163 gates a
`nameplate_display_name` (PlayerConfig assembly), while the scene_loader primitive-player path at
line 629 gates the SAME PlayerConfig field but WITH the full helper — direct evidence the two
player paths intentionally differ today.
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

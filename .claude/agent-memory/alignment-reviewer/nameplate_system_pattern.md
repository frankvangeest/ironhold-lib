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

**v2 runtime toggle (`ToggleOwnNameplate`, shipped 2026-07-03):** unit Action `Action::ToggleOwnNameplate`
(schema/actions.rs ~77, after SyncAudioState) flips resource `PlayerNameplatePreference(pub bool)`
(nameplate.rs ~60, `init_resource` lib.rs ~156) and emits `nameplate.own_shown`/`nameplate.own_hidden`
(two distinct events, mirrors ToggleMute's audio.muted/audio.unmuted — correct precedent). Executor arm
action_executor.rs ~307 is a pure resource-flip + GameEvent emit, NO ActionQueue push (correct — same
shape as ToggleMute). `nameplate_pref: ResMut<PlayerNameplatePreference>` added to `SceneStateParams`
bundle (mod.rs ~433, next to audio_state). Consumed ONLY by nameplate_visibility_system's per-frame
Player branch (~222): the previously-combined `prefab_override==Some(true) || player_q.contains(entity)`
condition is now SPLIT — Some(true) is distance-only (ignores pref), no-override Player is
`!nameplate_pref.0 || beyond max_distance`. Precedence verified in code: per-prefab Some(true)/Some(false)
> PlayerNameplatePreference > scene show_player_nameplate default. Re-seeded from show_player_nameplate
at scene_loader.rs ~1166 (does NOT persist across scene transitions — deliberate, matches player_enabled
not AudioState's session-persistence). NOTE: unlike AudioState (two project_loader insert sites), nameplate
config has ONE seeding site (scene_loader ~1166-1171) — spawn_scene_v2 is the sole path; do not expect a
second. CLI touchpoint REQUIRED: query.rs action_kind ~611 (exhaustive match — omitting breaks
`cargo check -p ironhold_cli`, the mandatory check that catches it). Tests that build NameplateSceneConfig
directly (bypassing spawn_scene_v2) must also `insert_resource(PlayerNameplatePreference(...))` or it sits
at Default=false and hides the player — this regressed test_nameplate_visibility_player_bypasses_faction_filter.

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

**Nameplates now inherit `label_depth_scale` (`feature/nameplate-zoom-spacing`, 2026-08).**
`nameplate_setup_system` gained `Res<LoadedLabelDepthScale>` and resolves `depth_scale` ONCE per
batch (not per anchor) via `resolve_label_depth_scale(res.0.as_ref(), None)` instead of the old
hardcoded `None`. Because a nameplate anchor carries no `TextFont` of its own, the compensation is
applied as `Transform.scale` on the anchor (new branch in `world_label_screen_pos_system`) rather
than as a `TextFont.font_size` rewrite — see [[label-depth-scale-pattern]] for the full branch/
override matrix and the three anchor styles still hardcoded to `None`.

**Split-screen distance-culling fix (Phase 3 of split_screen_camera_followups, 2026-07-12):**
`nameplate_visibility_system` no longer queries cameras at all. A new INTERNAL-ONLY component
`NameplateCameraDistance(pub Option<f32>)` (nameplate.rs) is attached to each anchor in
`nameplate_setup_system` (beside NameplateAnchorWidget). `world_label_screen_pos_system` (lib.rs
~522) — which already selects the one authoritative active camera per WorldLabel — now stashes
`(world_pos - selected_cam.translation()).length()` onto that component (Option<&mut> in its query,
graceful when absent), clearing it to `None` on every early-return (tracked entity gone/hidden, no
qualifying camera). visibility_system reads it via `dist_q.get(anchor.0)` and treats `None` as
out-of-range (matches prior `.single()` no-op contract). This is a "store-and-read" pattern that
guarantees the two systems agree on which camera is authoritative — the `.after(
world_label_screen_pos_system)` ordering (lib.rs ~308) is now load-bearing for correctness, not
just the two-writer visibility contract. NameplateCameraDistance is NOT designer-facing (correct —
same class as PlayerOwnership; culling threshold stays the designer-authored
NameplateOptionsDef.max_distance). Pure runtime bugfix, no schema/RON/CLI surface change.

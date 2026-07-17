---
name: player-stat-widgets-pattern
description: How stat_label/world_stat_bar reach player prefabs (closes the per_player_stat_pools footgun); shared spawn helpers in stat_display.rs; generic {self}.<stat>-with-no-template warn+CLI check across all prefab kinds
metadata:
  type: project
---

Reviewed 2026-07-17 (`feature/player-stat-widgets`, ALIGNED, no blocking issues). Closes the
footgun documented in [[per-player-stat-pools-pattern]] (player spawn path silently dropped
`stat_label`/`world_stat_bar`) and resolves the triplication refactor candidate in
[[world-label-stat-ui-pattern]].

**Part A — de-duplication (pure refactor).** Widget-entity construction extracted from 3 sites
(scene_loader.rs two Phase-B loops + `drain_dynamic_stat_ui_system`) into `pub` helpers in
`capabilities/stat_display.rs`: `spawn_stat_label_widget(commands, tracked, stat_key, def, &ctx)`
and `spawn_world_stat_bar_widget(commands, tracked, stat_key, def, &mut ctx)`, taking
`StatWidgetSpawnCtx { meshes, color_materials: Option<&mut Assets<ColorMaterial>>, depth_scale:
Option<(f32,f32)>, is_split_screen }`. Each call site still resolves depth_scale/is_split_screen
itself (scene: `scene.label_depth_scale` + captured `is_split_screen`; dynamic:
`LoadedLabelDepthScale` + `active_split||dynamic_split`) — helper is source-agnostic. When
reviewing a widget-spawn change, verify these per-site sources stay distinct.

**Part B — players routed through the EXISTING dynamic queue, not a new path.** `PlayerConfig`
gained `stat_label: Option<StatLabelDef>` / `world_stat_bar: Option<WorldStatBarDef>` (`schema/player.rs`,
`#[serde(default)]`; PlayerConfig is Rust-assembled, never deserialized from scene RON → purely
additive). Forwarded in `assemble_player_config` (entity_spawner.rs ~1027). All FOUR player spawn
paths push a `{self}`-resolved `DynamicStatUiEntry` onto `DynamicStatUiQueue`, drained next frame by
`drain_dynamic_stat_ui_system`:
- GLB immediate + GLB terrain-delayed → both via `spawn_player_entity_core` (~863), `{self}` vs
  `player_config.spawn_id`. `DynamicStatUiQueue` threaded through `spawn_player_entity` /
  `spawn_players_and_camera` / `spawn_player_when_terrain_ready`, and bundled into `SceneV2Params`
  for `spawn_scene_v2` (that system is at Bevy's 16-param SystemParam ceiling — a bare 17th param
  is a COMPILE error).
- Primitive/capsule inline (scene_loader.rs ~811) → re-fetches prefab by key from
  `prefab_catalog` (NOT via PlayerConfig), `{self}` vs `entity_id`. This is a deliberate
  divergence: GLB reads `PlayerConfig.stat_label`, primitive reads `prefab.stat_label` directly.
  Both ultimately read the same prefab, so correct today, but a future transform in
  `assemble_player_config` would NOT reach the primitive path. Accepted (single-path player
  construction unification is explicitly out-of-scope, tracked in claude_suggestions).
- Action::Spawn (tags:["player"] prefab) → `drain_spawn_queue_system` generic push (~393).

**Part C — generic contradictory-intent guard (the reusable pattern here).** A `{self}.<stat>`
widget key with no matching `stat_templates` entry on the SAME prefab renders empty silently.
Guarded in TWO places, both generic across every prefab kind (NOT player-specific):
- Scene-load `warn!`: `warn_missing_stat_widget_templates(scene, prefab_catalog)` iterates
  `scene.entities` (players ARE scene.entities entries — `is_player` is derived per entity_def in
  the spawn loop, so players are covered identically to NPCs).
- CLI `validate.rs`: `missing_stat_widget_template` error_type, iterates `&catalog.prefabs` (EVERY
  prefab, even unreferenced ones — broader than the scene-load warn).
Both `strip_prefix("{self}.")` → skip global keys (no false positives) → check
`stat_templates.iter().any(|t| t.key == local_stat)`. Bare `{self}` (no dot) is not caught by
either (unlikely authoring; pre-existing shape).

**Verification checklist that passed:** no dead fields (both PlayerConfig fields read); `{self}`
resolves against the same id the SpawnId component gets on every path; playtest aid
(local_coop_demo player_p1_split/p2_split each declare a `mana` template + Ascii `{self}.mana`
world_stat_bar — no Part C warning). Pixel-style bars remain single-viewport (not rank-duplicated)
— correct per split-screen limitation.

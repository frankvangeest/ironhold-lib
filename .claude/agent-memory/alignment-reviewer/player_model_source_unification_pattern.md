---
name: player-model-source-unification-pattern
description: v1 unifies GLB + primitive player body construction through spawn_player_entity_core via PlayerConfig.model_source; only the immediate non-terrain scene-load path; terrain + character-select are v3-deferred with warn!/validate
metadata:
  type: project
---

Reviewed 2026-07-19 (`feature/player-model-source-unification`, v1, ALIGNED). Collapses the old
~165-line inline primitive-player block in `scene_loader.rs` into the shared player pipeline.
Closes the footgun in [[player-spawn-via-action-pattern]] and [[player-stat-widgets-pattern]]:
a `kind: Primitive` player prefab tagged `["player"]` previously bypassed `PlayerConfig` entirely
and silently never got `PlayerIndex`, `material`, `StatMap`, or stat widgets even when authored.

**The dispatch.** `assemble_player_config` (entity_spawner.rs ~1104, the single source of truth
for both scene-load and Action::Spawn) branches on `prefab.kind == PrefabKind::Primitive` (NOT
`shape`/`children` presence — a valid primitive may have `shape:None` defaulting to Capsule3d and
empty children) → `PlayerModelSource::Primitive{shape,params,children}` else
`PlayerModelSource::Glb(model)`. `PlayerConfig.model_path: String` became
`model_source: PlayerModelSource` (schema/player.rs). PlayerConfig is Rust-assembled, never
deserialized from RON, so PlayerModelSource has no Deserialize — purely additive.
`spawn_player_entity_core` matches on `model_source` for body construction ONLY; everything after
(PlayerIndex, StatMap, material override, nameplate, stat widgets) is shared unconditionally —
that sharing IS the feature.

**v1 scope = immediate non-terrain scene-load path only.** Primitive body construction needs
mesh/material `Assets`, the per-scene-load `mats.built.0` map, `primitive_default_color`, and
catalogs — bundled into a new `PrimitivePlayerCtx` (child_ctx: ChildSpawnCtx + prefab_catalog +
load_errors) threaded ONLY through `spawn_players_and_camera` from `spawn_scene_v2`. The other
two `spawn_player_entity_core` callers pass `None`; a `Primitive` model_source with no ctx
`.expect()` panics (v1-scope invariant, only scene-load builds Primitive configs).

**Two v3-deferred rejection paths — both diagnosable (a review focus, PASS):**
- terrain + primitive player: scene_loader.rs warn! + skip when `scene.terrain.is_some()`, MIRRORED
  by validate.rs cross-file error `unsupported_primitive_player_on_terrain` (design-time).
- character-select (Action::Spawn primitive player): action_executor.rs specific warn! at the
  `asset_catalog.models.get(prefab.model)` empty-string lookup failure (was generic "model key not
  found"). Runtime-only — NO validate check exists for this case (could be a future addition).

**WARNING found (non-blocking, logged): load_errors diagnosability gap.** `load_errors` is
error!()-logged at scene_loader.rs ~700, but `PrimitivePlayerCtx.load_errors` is passed by &mut
into the spawn call AFTER that (~759). A primitive player with cosmetic `children` referencing a
missing nested prefab / cycle pushes into load_errors post-log → silently swallowed. Not exercised
by the demo (player_p1/p2_primitive use a bare `primitive:` capsule, no children). Fix: re-log
load_errors after the player spawn block.

**Demo (playtest aid):** local_coop_demo room7 = 2 primitive-capsule players, vertical split
(player_p1_primitive index0/tint_blue/mana100, player_p2_primitive index1/tint_red/mana60), each
with `{self}.mana` Pixel world_stat_bar. Proves distinct PlayerIndex/material/StatMap per instance.
Portals: room6↔room7 reuse portal_to_room6 event + existing rules; new portal_to_room7 prefab.
Material override reaches the primitive body mesh child because `apply_material_overrides`
(material_factory.rs) walks `iter_descendants` of the player root.

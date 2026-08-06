---
name: player-model-source-unification-pattern
description: v1 unifies GLB + primitive player body construction via PlayerConfig.model_source (scene-load path only; terrain/char-select v3-deferred); v2 unifies zero-Friction across both sources and ships the first player prefab using cosmetic `children:`
metadata:
  type: project
---

Reviewed 2026-07-19 (v1, ALIGNED) and 2026-08-06 (v2, ALIGNED). Collapses the old ~165-line
inline primitive-player block in `scene_loader.rs` into the shared player pipeline.
Closes the footgun in [[player-spawn-via-action-pattern]] and [[player-stat-widgets-pattern]]:
a `kind: Primitive` player prefab tagged `["player"]` previously bypassed `PlayerConfig` entirely
and silently never got `PlayerIndex`, `material`, `StatMap`, or stat widgets even when authored.

**The dispatch.** `assemble_player_config` (entity_spawner.rs ~1247, the single source of truth
for both scene-load and Action::Spawn) branches on `prefab.kind == PrefabKind::Primitive` (NOT
`shape`/`children` presence — a valid primitive may have `shape:None` defaulting to Capsule3d and
empty children) → `PlayerModelSource::Primitive{shape,params,children}` else
`PlayerModelSource::Glb(model)`. `PlayerConfig.model_source` replaced `model_path: String`
(schema/player.rs). PlayerConfig is Rust-assembled, never deserialized from RON, so
PlayerModelSource has no Deserialize — purely additive. `spawn_player_entity_core` matches on
`model_source` for body construction ONLY; everything after (PlayerIndex, StatMap, material
override, nameplate, stat widgets, collider, Friction) is shared unconditionally — that sharing IS
the feature.

**v1 scope = immediate non-terrain scene-load path only.** Primitive body construction needs
mesh/material `Assets`, the per-scene-load `mats.built.0` map, `primitive_default_color`, and
catalogs — bundled into `PrimitivePlayerCtx` (child_ctx: ChildSpawnCtx + prefab_catalog +
load_errors) threaded ONLY through `spawn_players_and_camera` from `spawn_scene_v2`. The other
`spawn_player_entity_core` callers pass `None`; a `Primitive` model_source with no ctx `.expect()`
panics (scope invariant). Three diagnosable rejection paths for primitive players:
terrain (scene_loader warn! + validate error `unsupported_primitive_player_on_terrain`),
character-select `Action::Spawn` (runtime warn! only, no validate check), and
`join_prefab_keys` hot-join (action_executor.rs warn! on `kind: Primitive`).

**v2 (2026-08-06) = zero-RON-surface Friction unification + room10 mixed demo.** The only Rust
change is deleting the `PlayerModelSource::Primitive`-only guard so
`Friction { coefficient: 0.0, combine_rule: Min }` is inserted in the same shared tuple as
`Collider::compound` for every player. No schema field, no action, no asset key. Friction is
deliberately NOT designer-authorable (documented in docs/20_data_formats.md ~1896 and in the
`idle_drag` row ~2058); the RON knob for the one behavior it affected (idle creep on slopes) is
`MovementConfig.idle_drag`.

**FOOTGUN — `idle_drag` is applied airborne too** (`capabilities/player.rs` ~240, no
`is_grounded` gate). Docs recommend lowering `idle_drag` to stop downhill creep without saying that
a near-zero value also kills horizontal air momentum when input is released mid-jump. Any future
slope/friction guidance should mention this tradeoff.

**FOOTGUN — `material:` on a composed primitive player flattens all child colors.**
`apply_material_overrides` (runtime/material_factory.rs ~198) applies the built material to EVERY
`Mesh3d` under `iter_descendants(root)`, so a prefab that authors both `material: "tint_x"` and
per-child `primitive: (color: ...)` in `children:` loses every child color/roughness/metallic.
`local_coop_demo`'s `player_p2_primitive_split_ring` does exactly this (head/shoulder tones are
dead data; only the silhouette differentiates). Fix is RON-only (drop `material:` and color the
torso via `primitive.color`, or drop the child colors) — never a blocker, but call it out.

**FOOTGUN (still open) — player-path `load_errors` are logged before the player spawns.**
`scene_loader.rs` error!()s `load_errors` at ~732 but passes `&mut load_errors` into
`PrimitivePlayerCtx` at ~791. A player prefab whose `children:` reference a missing/cyclic nested
prefab pushes post-log → silent. Compounded: `crates/ironhold_cli/src/commands/validate.rs` has NO
check for `children[].prefab` references at all (grep "children" → no matches), so neither
design-time nor runtime surfaces it. v2 made this reachable in practice (first shipped player
prefab with `children:`, and docs now advertise the pattern). Fix: re-log after the player block.

**Demos.** room7 = 2 primitive-capsule players (minimal cap-removal regression baseline, do not
retrofit). room10 (v2) = mixed pair: GLB `player_p1_split_ring` (owns the sole
`camera.split.own_viewport_only` switch) + primitive `player_p2_primitive_split_ring` with
identical input wiring, `gamepad_index: 1` (mandatory — `gamepad_key_without_gamepad_index` is a
hard validate failure), cosmetic `children:` body, `tint_steel` material, per-player mana action
bars, and two `cube_obstacle_room10` blocks as the friction-comparison aid. `local_coop_demo` has
no per-scene screenshot baselines for room6/8/9/10 — partial coverage is the established norm there.

**Friction playtest surface beyond the demo:** removing collider friction changes idle behavior for
existing shipped GLB players on slopes — `quick_scene` (fbm terrain, `player_warrior` authors no
`idle_drag`) and `terrain_demo` are the concrete cases; the plan's fallback if creep appears is a
low non-zero coefficient (e.g. 0.15, still `Min`), not reverting to primitive-only.

---
name: assemble-player-config-primitive-panic
description: spawn_player_entity_core's Primitive arm .expect()s a PrimitivePlayerCtx; Action::Spawn can build a Primitive config when a primitive prefab has a resolvable model key, crashing
metadata:
  type: project
---

`spawn_player_entity_core` (entity_spawner.rs) dispatches body construction on
`PlayerConfig.model_source`. The `Primitive` arm does `primitive_ctx.expect(...)` — a hard panic
if the ctx is `None`. Only the immediate scene-load path (`spawn_scene_v2` → `spawn_players_and_camera`)
passes `Some(ctx)`; the terrain-deferred and dynamic `Action::Spawn` paths pass `None`.

**The invariant "only scene-load builds Primitive configs" is enforced only partially.** In
`action_executor.rs`, a player-tagged prefab is assembled into a `PlayerConfig` only after the
`asset_catalog.models.get(&prefab_def.model)` lookup succeeds. The author assumed a primitive
prefab always has `model == ""` (see `catalog.rs`: `pub model: String, // empty for Primitive` —
a **convention comment, not schema-enforced**), so the empty-model lookup fails and rejects it.
But a `kind: Primitive`, `tags:["player"]` prefab with a *non-empty, resolvable* `model` key
sails past the lookup → `assemble_player_config` builds `PlayerModelSource::Primitive` (ignoring
model_path) → drain_spawn_queue_system → `spawn_player_entity(None)` → **panic**.

**Why:** the rejection guard checks the *failure* branch of the model lookup only; it never
checks `kind == Primitive && player-tag` on the *success* branch. Introduced by
player_model_source_unification v1 (2026-07-19); pre-feature this same prefab spawned a GLB body
(weird but no crash).

**How to apply:** when reviewing player-spawn or `assemble_player_config` changes, remember an
`.expect()`/`unwrap` guarding a "which caller built this" invariant is fragile because
`assemble_player_config` has 2 callers (scene_loader + action_executor) and only one supplies the
primitive ctx. Any latent panic here won't be caught by `cargo test` (no test drives Action::Spawn
on a primitive-model-bearing player prefab) nor by `ironhold_cli validate` (its new check only
covers primitive-player + terrain, not primitive-player + Action::Spawn). Related:
[[stat_display_changedetection_asmut]] is another "guard defeated by a subtler path" case.

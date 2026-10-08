# Player-construction sites

Every place a player entity is constructed, and what a new "every player gets X" change has to touch.

> Moved from `crates/ironhold_core/src/CLAUDE.md` on 2026-10-08 (the core CLAUDE.md split). The hard rules for this area stay in the folder `CLAUDE.md` files; this page keeps the long-form reference and history verbatim.

## Player-construction sites inventory (1-5) + post-dispatch rule

<!-- b:1121.ref -->

### Player-construction sites

Any feature that changes player spawning (local co-op, character select, respawn, possession)
must account for all of these or players diverge silently. Before
`player_model_source_unification.md` v1, this was **four** genuinely separate sites — one of
them (the primitive/capsule path) bypassed `PlayerConfig` entirely and silently lacked
`PlayerIndex`/material override/`StatMap`. v1 collapsed that gap for the common case; what
remains is:

1. **Unified scene-load collector** — `scene_loader.rs` builds `player_configs: Vec<PlayerConfig>`
   from every scene entity whose prefab has `tags: ["player"]`, **for both GLB (`kind: Actor`) and
   primitive (`kind: Primitive`) prefabs**, via the shared `assemble_player_config()` helper
   (`entity_spawner.rs`) — dispatched on `prefab.kind == PrefabKind::Primitive`, not on
   `shape`/`children` presence. This is the only collector now; the old separate
   primitive-collector-plus-inline-spawn path is gone for the non-terrain case.
2. **Dynamic spawn** — `action_executor.rs`'s `Action::Spawn` handler assembles a `PlayerConfig`
   for a `tags: ["player"]` prefab (the character-select flow). **GLB-only in practice**: a
   primitive-shaped player prefab has no `model` key (empty string), so the
   `asset_catalog.models.get(&prefab_def.model)` lookup fails and rejects it with a `warn!` before
   `assemble_player_config` is ever reached — v3-deferred, not a v1 regression (primitive players
   never worked here).
3. **Terrain-deferred spawn** — `spawn_delayed_players_system` via `PendingPlayerConfig`
   (`Vec<PlayerConfig>`). Also **GLB-only in practice**: a primitive player prefab combined with
   `scene.terrain: Some(...)` gets a scene-load `warn!` and an `ironhold_cli validate` error
   (`unsupported_primitive_player_on_terrain`) instead of spawning — v3-deferred, since the
   built-materials map/mesh-asset access primitive body construction needs isn't yet threaded
   through this resource-poor path (see `planning/features/player_model_source_unification.md`'s
   v3 section).
4. **Shared spawn functions** — `entity_spawner.rs`'s `spawn_player_entity` (single player, own
   `ActiveCameraMode::Orbit`), `spawn_players_and_camera` (1+ players; 2+ share one camera or split-screen),
   and `spawn_player_when_terrain_ready`. Only `spawn_players_and_camera` takes a `CameraSpawnMode`
   (`Spawn`/`Suppressed`) parameter — when a scene also has a `tags: ["flycam"]` entity, both its
   call sites (scene load, and `spawn_player_when_terrain_ready` reading the `SuppressPlayerCameras`
   resource) pass `Suppressed`, skipping every camera resource insert/spawn after the player-entity
   loop (see `planning/features/flycam_scene_conflicts.md`). `spawn_player_entity` (site 1, dynamic
   `Action::Spawn`/character-select) and the hot-join path (site 5, later on this page) do **not** check
   `SuppressPlayerCameras` — a player dynamically spawned at runtime always gets its own camera,
   even in a scene that started in spectator mode; a known, documented limitation, not an oversight.
   All three call the private `spawn_player_entity_core`,
   which dispatches body construction on `PlayerConfig.model_source: PlayerModelSource` (`Glb(key)`
   or `Primitive { shape, params, children }`) — everything **after** that dispatch (physics
   bundle, `tag_spawned_entity`, `PlayerIndex`/`PlayerOwnership`/`PlayerTarget`/`BoundGamepad`,
   `StatMap`, stat widgets, nameplate) is now shared, unconditional code for both model sources, not
   a GLB-only path. `BoundGamepad(player_config.bound_gamepad)` is the one field here that isn't
   always `None` — see site 5, later on this page. Only site 1 passes a real `PrimitivePlayerCtx` (mesh/material
   assets, prefab catalog, built-materials map) so the `Primitive` arm can actually build a body;
   sites 2 and 3 pass `None` and would panic if they ever reached the `Primitive` arm — which they
   can't, since both reject primitive-shaped prefabs earlier (see sites 2 and 3).
5. **Hot-join spawn** (`local_coop_hot_join_leave.md`) — `action_executor.rs`'s `Action::JoinPlayer`
   arm assembles a `PlayerConfig` via the same `assemble_player_config()` helper as site 2, then
   overrides `PlayerIndex` to the target slot, sets `PlayerConfig.bound_gamepad` directly from any
   captured `PendingJoinGamepad` (see `docs/dev/gamepad-routing.md`, "Gamepad-triggered hot join"), and pushes a
   `QueuedSpawn` with `is_hot_join: true`.
   `drain_spawn_queue_system`'s `is_hot_join` branch calls `spawn_player_entity_core` directly
   (camera-less, `PrimitivePlayerCtx: None` — **GLB-only**, same reasoning as site 2) followed by
   `spawn_split_camera_for_player` (a thin wrapper factored out of `spawn_players_and_camera`'s
   `Grid` loop, adding just `SplitViewportSlot`/`Camera.order`), then increments
   `ActiveSplitSlotCount` by one. Scoped to `Grid`-split scenes only — see the doc comment on
   `ActiveSplitSlotCount` (see `docs/dev/split-screen-cameras-and-widgets.md`), which this site resolves. See `docs/dev/gamepad-routing.md`, "Gamepad-triggered hot join",
   for the `bound_gamepad` hand-off `PlayerConfig.bound_gamepad` (set in `action_executor.rs`'s `Action::JoinPlayer` arm from `pending_join_gamepad.0.take()`) feeds into.

Because `PlayerIndex`, `PlayerTarget`, `BoundGamepad`, `StatMap` (when `stat_templates` is
non-empty), stat widgets, and material override are now inserted in the shared post-dispatch code
rather than per-model-source, **a new "every player gets X" component only needs adding in one
place**
(`spawn_player_entity_core`, after the model-source match) instead of being checked against
multiple divergent spawn paths — this is the exact class of bug the old inventory of separate player-construction sites on this page
existed to flag, and the risk surface for it is now much smaller. What still needs checking
against multiple paths: whether a *new* `PlayerConfig`/`PrefabDef` field is forwarded correctly in
`assemble_player_config`'s two call sites (sites 1 and 2), and whether a fix belongs in the
model-source-dispatch match (body-construction-specific) vs. the shared post-match code
(everything else).

Note `PlayerIndex` can still be entirely absent from an entity in principle — "primary player" is
defined as "`PlayerIndex(0)` **or no `PlayerIndex` at all**" (`capabilities/targeting.rs::
is_primary_player`) — but as of v1 this is no longer reachable via any *spawning* primitive
player; it's only meaningful for the v3-deferred terrain/character-select paths, which don't spawn
primitive players at all yet (sites 2/3 reject them outright, they don't spawn one without a
`PlayerIndex`).

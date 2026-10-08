# runtime/scene_manager/ — scene loading, the FSM interpreters and the action executor

Rules for the files in this folder (`scene_loader.rs`, `entity_spawner.rs`, `action_executor.rs`,
`action_substitution.rs`, `project_loader.rs`, `mod.rs` and the interpreter systems). Loaded when you touch a file
here; the crate-wide rules are in `../../CLAUDE.md`, the other `runtime/` files have `../CLAUDE.md`.

## Topic references

- Every place a player entity is constructed, and what a new "every player gets X" component has to touch:
  `docs/dev/player-spawn-sites.md`.
- Lootable corpses, container panels and the respawn loop: `docs/dev/lootable-corpse.md`.
- Gamepad claiming and hot join: `docs/dev/gamepad-routing.md` (the rules are in `../CLAUDE.md`).

<!-- b:70 -->
## Conditions on rules: only `LogicState`

The only runtime condition the FSM has is `LogicState`, a single named string; a binding in a state's `on:` list fires
only while the FSM is in that state. To add a condition: have a gameplay system emit a `GameEvent` (like
`stat_threshold_system`'s `"hp.low"`), add a `transitions` entry (`( from: None, on: "hp.low", to: "hp_low" )`;
`from: None` matches any state), and put the conditional logic in that state's `on:` list or its `entry_actions`.

**Never change `LogicState` from a gameplay system directly** (there is no `Action::EnterState`; it was removed in
2026-09): only a transition's firing runs the destination's `entry_actions` and the source's `exit_actions`, and forcing
the state skipped both, a silent-desync bug class. Do not add a general condition system to the interpreter unless this
pattern is genuinely insufficient. Worked example: the "Removed: rules.ron" callout in `docs/20_data_formats.md`.

<!-- b:111 -->
## Entity FSM and the interpreter chain

Per-entity behavior uses the same `StateMachineAsset` schema as the global FSM (files in `behaviors/`). In behavior
files `{self}` in any event pattern or action target is replaced by the entity's spawn id. The `Update` chain is
`fsm_interpreter_system` -> `entity_fsm_interpreter_system` -> `flush_pending_intent_system` (flushes action-bar slot
actions not suppressed by a matched intent) -> `action_executor_system`.

**Never bypass the pipeline from entity behavior.** Entry/exit actions in a `.behavior.ron` push to the global
`ActionQueue` and go through the same executor as everything else. Do not give the entity FSM interpreter `Commands`
access.

<!-- b:188 -->
## `{target}` substitution

`{target}` in any action field (key, entity, event, id, spawn_point) is replaced by the current `CurrentTarget` spawn
id, in every interpreter system and in the action bar's own intent handling, before the action reaches `ActionQueue`. If
`CurrentTarget` is `None` the literal `"{target}"` stays and the action usually no-ops. The substitution sites are listed
in `../../schema/CLAUDE.md` (four sites; a new field must be handled at all of them).

<!-- b:144 -->
## `{new_id}` on `Spawn.id`

`{new_id}` works only in `Spawn`'s `id` field, and unlike `{self}`/`{target}` it is **resolved by `action_executor.rs`'s
`Action::Spawn` arm**, not by the interpreters: that arm is the only place with mutable access to the counter
(`SpawnRegistry.counter`, shared with the auto-generated id used when `id` is omitted), and resolving downstream of every
interpreter and dialogue makes it work whichever one queued the action. Deliberate, not an inconsistency.

- Use it to compose an id that will not collide with an earlier spawn from the same source, e.g.
  `Spawn(prefab: "...", id: "{self}_corpse_{new_id}")`. **It is not an absolute uniqueness guarantee:** a hand-authored
  literal id of the same shape can still collide, since `SpawnRegistry.entities` is one flat namespace. The executor warns
  if the resolved id is already registered, and if it still contains a literal `{` (a typo'd `{new_id}`, or `{self}`/
  `{target}` authored where nothing resolves them).
- Repeated `{new_id}` in one `id` resolve to the same value (one id per spawn). The counter resets to 0 on `LoadScene`.
- **The resolved id is not observable from other RON files.** Only the spawned entity's own behavior file can reach it
  (via `{self}`), or whatever holds `CurrentTarget` (via `{target}`); a literal `Despawn("thing_{new_id}")` in
  `state_machine.ron` never resolves. Use `{new_id}` only for entities that manage their own lifetime.
- `{self}` also resolves inside a dialogue choice's `Spawn.id`/`Spawn.spawn_point`
  (`capabilities/dialogue.rs::substitute_self_in_action`); `{new_id}` resolves there too, later, at the executor.

<!-- b:92 -->
## Composite and nested prefab spawning

All child spawning goes through `spawn_primitive_children` (`scene_loader.rs`); **never duplicate its mesh/material
dispatch arms** (the two callers are composite non-player prefabs and player cosmetic children). It handles inline
primitive children and nested prefab references (`ChildPrimitiveDef.prefab`), dispatching on `nested_prefab.kind`:
`Primitive` with `children` spawns an anchor and recurses; `Primitive` without children spawns an anchor plus one mesh
child (`build_primitive_mesh`); `Actor`/`Prop` calls `spawn_prefab_instance` and parents the result at the child's
`offset`/`rotation_euler_deg`/`scale`. `ChildSpawnCtx` carries `item_catalog` so a nested prefab's
`attach_prefab_features` stacks `inventory.initial_items` against the real `ItemCatalog`, not a `max_stack: 99` fallback.

Cycle detection at load time is `PrefabCatalog::validate()` (`prefab_has_cycle()`); the spawn-time depth limit is 8.
Transform composition is multiplicative, so non-uniform scale on a parent anchor shears rotated children (say so in RON
comments where relevant). The `behavior` field works on every prefab kind, including composite primitives.

<!-- b:464 -->
## `behavior` on composite primitive prefabs

Both the single-mesh and the composite (multi-child) primitive paths in `scene_loader.rs` attach `PendingBehavior`.

<!-- b:489 -->
## `Spawn.at_entity` copies a live entity's transform

`Action::Spawn.at_entity` resolves position **and facing, rotation and scale** from a live entity's `GlobalTransform`
(`compute_transform()`), via the same `SpawnRegistry` lookup `SpawnEffect.entity` uses; `{self}`/`{target}` work in it.
**If the entity cannot be resolved and no `position`/`spawn_point` fallback was given, the spawn is skipped with a
warning, never placed at the origin** (an important dynamic entity such as a lootable corpse at `(0,0,0)` is worse than
none). The transform is resolved at executor time into `QueuedSpawn.transform`, before `drain_spawn_queue_system`
reads it, so a same-frame `Despawn("{self}")` after it can never race it.

<!-- b:554 -->
## `Action::Despawn` closes an open container panel

When the despawned entity is the open container, `Action::Despawn` runs the same teardown `CloseContainer` does.
Without it, `LoadedContainerUi.active_container` points at a gone entity and `panels_open` stays non-zero, permanently
blocking interact, pickup and tab-targeting.

<!-- b:587 -->
## `OpenContainer` must not double-count `panels_open`

`interactable_system` fires `entity.interacted` for **every** interactable in radius on one keypress, so two nearby
containers can both queue `OpenContainer` in one frame. The single `ContainerPanel` shows one container, so a second
`OpenContainer` while one is open only re-targets `active_container` and does not increment `panels_open` again; the
counter is decremented once per `CloseContainer`, and an over-increment permanently suppresses interact, pickup and
tab-targeting (all gated on `panels_open == 0`, see `capabilities/inventory.rs`) until the next `LoadScene`.

<!-- b:598 -->
## Do not add `trigger_zone` to a prefab with a Dynamic rigid body

A `trigger_zone` sensor gets no `ColliderMassProperties` override at spawn (`entity_spawner.rs`,
`attach_prefab_features`), so its volume-derived mass folds into the whole rigid body's mass on a Dynamic (NPC) body and
makes it wildly heavy and effectively unpushable. Existing uses are safe only because they are `Fixed`-body Props
(chests, merchants, corpses). A general engine bug, tracked in `planning/backlog.md`.

<!-- b:1060 -->
## `audio_volume_var_system`

Mirrors `AudioState.active_fraction` into `GameVariables[AUDIO_VOLUME_PERCENT_KEY]` (`"audio_volume_percent"`, the
chosen preset as an integer string, not the effective volume) whenever `AudioState` changes, so a bound `Label` shows
the live volume with no RON rules. Scheduled `.after(action_executor_system).before(update_dynamic_labels_system)` (the
FSM interpreter never reads `GameVariables`, so `.before(fsm_interpreter_system)` would only add a frame of lag). Mute
is deliberately **not** mirrored: it still goes through the `audio.muted`/`audio.unmuted` RON `global_on` bridge. A test
standing in for the project loader's `AudioState` re-insert must insert a non-default `active_fraction`, or it passes
vacuously (the harness never completes a project load).

<!-- b:1053 -->
## Audio preloading

`preload_audio_system` runs on every `SceneEvent::Ready` and calls `asset_server.load::<AudioSource>()` for every
`LoadedAssetCatalog.audio` entry, so `Action::PlaySound` resolves without file I/O. **The handles stored in
`LoadedAudioHandles` must stay alive** or the asset server evicts the audio between scenes; the resource is cleared and
repopulated on each `Ready`, so it never accumulates stale handles.

<!-- b:1067 -->
## `tag_spawned_entity` is the single spawn-metadata source

Every addressable spawned entity gets its metadata from `tag_spawned_entity` (`mod.rs`): GLB actor/prop, single-mesh and
composite primitive, foliage root, every player spawn path and dynamic `Action::Spawn`. It always inserts `SpawnId` +
`PrefabKey` + `LevelEntity`, registers the entity in `SpawnRegistry`, and inserts `ClickSelectable`/`Targetable` per the
prefab flags (players pass `false`); player-specific components stay at the call site.

**Do not hand-insert `SpawnId`/`PrefabKey`/`LevelEntity` or call `spawn_registry.entities.insert` at a spawn site.** The
earlier divergent copies caused real bugs (GLB actors without `SpawnId`, dynamic spawns without `PrefabKey`/
`LevelEntity`). A new "every entity gets X" field means editing this helper once. `PrefabKey` (catalog key, e.g.
`"enemy_orc_melee"`) is not `SpawnId` (instance id, e.g. `"orc_01"`).

<!-- b:1201 -->
## Player `stat_label` / `world_stat_bar`

Players use the same floating-widget path as NPCs: `spawn_player_entity_core` pushes a `DynamicStatUiEntry` (with
`{self}` already resolved) onto `DynamicStatUiQueue` when `PlayerConfig.stat_label`/`.world_stat_bar` is set, for GLB and
primitive players alike. The widget entities are spawned by `capabilities/stat_display.rs`. `spawn_scene_v2` is at Bevy's
16-top-level-param `SystemParam` ceiling, so new resources it needs go into the existing `SceneV2Params` struct, not in
as bare params.

<!-- b:247 -->
## Every player entity must carry `PlayerTarget`

`action_bar_input_system` queries `(&SpawnId, &PlayerTarget, Option<&PlayerIndex>)` over `CharacterController` entities,
so a player built without `PlayerTarget` silently drops out of the match and **that player's whole action bar never
fires**. Every player-construction site inserts it today; re-check all of them whenever you touch player construction
(`docs/dev/player-spawn-sites.md`).

<!-- b:1224 -->
## Player `Friction` is set in two places that must agree

`spawn_player_entity_core` (`entity_spawner.rs`) inserts the initial `Friction` for every player, and
`player_movement_system` re-syncs `Friction.coefficient` every tick: `PLAYER_IDLE_FRICTION` (`0.15`, one shared constant
in `capabilities/player.rs`) only while `raw_grounded && !loco.moving`, `0.0` otherwise. **Never apply the non-zero
coefficient unconditionally:** Rapier's Coulomb friction at a wall contact resists the full tangent plane, so a live
coefficient while moving or airborne crushes jump height by up to ~83% and lets a falling player hang against a wall
(`tests/wall_friction_tests.rs`).

<!-- b:1213 -->
## A new rendering `PrefabDef` field must be forwarded on the player path

`spawn_player_entity_core` is separate from `spawn_prefab_instance`; a new rendering field (like `material`) must be
carried by `PlayerConfig` and forwarded by `assemble_player_config`. The rule and its reason are in
`../../schema/CLAUDE.md`.

<!-- b:1606.rule -->
## Hot join: the executor `.take()`s `PendingJoinGamepad`

`Action::JoinPlayer`'s arm (`action_executor.rs`) `.take()`s `PendingJoinGamepad` after resolving the joiner's
`PlayerConfig` and writes it into `PlayerConfig.bound_gamepad`; do not route it through `inputs.gamepad_index`. The full
rules are in `../CLAUDE.md`.

<!-- b:1646 -->
## Spawn queue

`Action::Spawn` **does not call `spawn_prefab_instance` inline**: it pushes a pre-resolved `QueuedSpawn` onto
`PendingEntitySpawns`, and `drain_spawn_queue_system` (end of the interpreter chain) processes at most
`SPAWNS_PER_FRAME = 2` per frame. This caps wave-spawn WebGPU pipeline-compile stalls (each new mesh+material
combination is a synchronous `createRenderPipeline()` of roughly 100-300 ms on WASM). Single spawns are transparent: push
and drain happen in the same `app.update()`. `PendingEntitySpawns` is cleared on `Action::LoadScene` so no orphaned spawn
runs after a transition.

<!-- b:1657.rule -->
## Component parity: dynamic spawns match scene-placed entities

A dynamically spawned entity gets the same prefab-driven components as a scene-placed one. **Nameplate gating has one
source of truth, `should_insert_nameplate(prefab.nameplate, show)` (`mod.rs`, beside `tag_spawned_entity`); never
re-inline the predicate at a new call site** (today `scene_loader.rs` x5, `entity_spawner.rs` x1, `action_executor.rs`
x1). `nameplate: Some(false)` always suppresses; otherwise `show` or an explicit `Some(true)` enables it. `show` differs
by entity type: NPCs/props use `scene.show_nameplates`/`nameplate_config.enabled`, `Player` entities use the independent
`show_player_nameplate`/`nameplate_config.player_enabled` and never `show_nameplates`.

<!-- b:1657.ref -->
- `motion`, `interactable`, `trigger_zone`, `colliders`, `stat_templates` and `behavior` are inserted inside
  `spawn_prefab_instance`, so scene and dynamic paths both get them. `stat_label`/`world_stat_bar` for a dynamic spawn go
  through `DynamicStatUiQueue` and `drain_dynamic_stat_ui_system` (one slot after `drain_spawn_queue_system`), a one-frame
  deferral.
- `nameplate_setup_system` queries `Added<NameplateTag>` every `Update` and re-checks `player_enabled` vs `enabled` per
  entity (`Option<&Player>`); `nameplate_visibility_system`'s `faction_filter` is NPC/prop-only (`Player` entities are
  distance-only).
- `Player`/`PlayerOwnership` (`capabilities/player.rs`) is inserted wherever a player spawns; `PlayerOwnership` is always
  `Local` today and reserved for LAN co-op.
- `Action::ToggleOwnNameplate` flips `PlayerNameplatePreference` (emits `nameplate.own_shown`/`own_hidden`); it is read
  only by `nameplate_visibility_system`, is re-seeded from `show_player_nameplate` on every scene load (it does not
  persist across scenes, deliberately), and an explicit per-prefab `nameplate: Some(..)` always wins.

<!-- b:1668.rule -->
## Label depth scale

`LoadedLabelDepthScale(Option<LabelDepthScaleDef>)` holds the scene's `label_depth_scale`. **`resolve_label_depth_scale`
(`mod.rs`) is the single resolver every consumer calls** (`drain_dynamic_stat_ui_system`, the scene-load stat widget
loops, `nameplate_setup_system`), so a wave-spawned widget, a scene-placed one and a nameplate shrink identically. There
is no per-widget override on `StatLabelDef`/`WorldStatBarDef`/`NameplateOptionsDef`: they always inherit the scene
setting. All four `world_stat_bar` styles scale: `Ascii` through the font-size branch of
`world_label_screen_pos_system`, the others and nameplates through the anchor's `Transform.scale` (XY only, Z untouched),
and both use one shared `depth_scale_factor()` so the curve cannot drift.

<!-- b:1668.ref -->
- Validation: `default_label_ref_distance()` (`schema/scene_v2.rs`) is `20.0`, matching `default_camera_config()`'s
  `max_radius` (that function is `pub` so `ironhold_cli` reuses the same numbers). `resolve_label_depth_scale` clamps
  `min_scale` to `[0.0, 1.0]` silently (per-widget call site, too hot to log); the diagnostics are `validate`'s
  out-of-range `min_scale` error and `--strict` `reference_distance` warning, and `scene_loader.rs`'s matching
  scene-load `warn!`s for designers without the CLI. `CameraModeDef::radius_range()` (`schema/camera.rs`) is the single
  source of truth for camera classification; both checks union `scene.join_prefab_keys` variants and skip player cameras
  when a `tags: ["flycam"]` entity is present.

# capabilities/ — gameplay systems

Rules for the systems in `crates/ironhold_core/src/capabilities/`. Loaded when you touch a file in this folder; the
crate-wide rules (schedule, determinism, `lib.rs` ordering edges) are in `../CLAUDE.md`. The long-form history and
reference for each area is in `docs/dev/`; read the matching page before changing that area.

## Topic references

- Player ground detection, jumping and coyote time: `docs/dev/player-ground-detection.md`.
- Split-screen cameras, `RenderLayers` rings, per-viewport labels and widgets: `docs/dev/split-screen-cameras-and-widgets.md`.
- Action-bar input (keyboard, gamepad, mouse click) and `{target}` routing: `docs/dev/action-bar-input-routing.md`.
- Animation resolver/playback pipeline and field ownership: `docs/dev/animation-pipeline.md`.
- Gamepad binding and hot join: `docs/dev/gamepad-routing.md`.

<!-- b:247 -->
## Every player needs `PlayerTarget`

`action_bar_input_system` queries `(&SpawnId, &PlayerTarget, Option<&PlayerIndex>)` over `CharacterController`, so a
player built without `PlayerTarget` silently drops out and its whole action bar never fires. Every
player-construction site inserts it; re-check all of them when touching player construction.

<!-- b:286 -->
## Targeting systems are ordered on purpose

`TargetingPlugin`'s `click_select_system` -> `tab_targeting_system` -> `target_auto_clear_system` are `.chain()`ed and
ordered `.before(action_bar_input_system)`; `target_indicator_system` and `target_hud_update_system` are ordered
`.after(target_auto_clear_system)`. **This is load-bearing:** they touch `PlayerTarget`/`CurrentTarget` with no data
dependency, so an unordered system races the action bar on the frame a target despawns and shows up as flaky tests.
**Any new system that reads or writes `PlayerTarget`/`CurrentTarget` in `Update` must be ordered explicitly against
this chain**; a passing test run does not prove an ordering, since the failure is a scheduling race.
The targeting-before-interpreters guarantee is transitive: it exists only because `ActionBarPlugin`'s
`(cooldown_tick_system, action_bar_input_system, action_bar_visual_system).chain().before(fsm_interpreter_system)`
(`action_bar.rs`) pulls the whole chain ahead of both interpreters. Removing or restructuring that edge silently drops
the guarantee, and no test asserts the schedule graph, so this note is the only guard.

<!-- b:284.rule -->
## Mouse-click action-bar slots

A slot click is a third fire source in `action_bar_input_system`, detected per entity as
`click_fired = i.is_changed() && *i == Interaction::Pressed` on `Option<Ref<Interaction>>` (the `Ref` change flag is
the edge detector). **The `Option` is load-bearing:** tests spawn a bare `ActionSlotUi`, and a required `Ref` would
silently drop them from the query. **Do not replace this with a `ClickedSlots`-style resource keyed by slot-key**: a
stale key re-fires every frame, survives `LoadScene`, and two bars sharing a key fire from one click. Click and key
collapse into one `keyboard_fired` decision (a click plus a key on one frame activates once), and a click acts for the
slot's own `owner_player` whichever viewport the cursor is in (the mouse is shared hardware). Clicks are ignored while
`InspectorEnabled`. In tests, write `Interaction::Pressed` once after a warm-up update, never every frame.

<!-- b:323 -->
## `target.*` events and selection

Selection is **screen-space proximity** (`camera.world_to_viewport`, nearest to the cursor), not mesh raycasting, which
hits bind-pose geometry and misses animated GLBs. Tab-cycle is nearest-first by world distance and runs for every
player independently; `click_select_system` maps the resolved camera to its owner via `CameraTargets` (the `Party` and
no-player cameras fall back to the primary player; one mouse acts for one player per click). `target.changed*` and
`target.cleared` fire **only for the primary player**, and the `target_display`/`target_name`/`target_id`
`GameVariables` go **blank whenever 2+ players exist** (a plain `CharacterController` count, not split-screen state): a
multi-player scene needs the per-viewport `target_hud:` block instead. `select_aim_height` on `PrefabDef` (default 1.0)
sets the click-projection height.

<!-- b:545 -->
## `target_auto_clear_system` clears on despawn

It must treat "not found in `SpawnRegistry`" the same as hidden: a capability that `Despawn`s a targeted entity
removes it from the registry outright, and a hidden-only check lets a stale `PlayerTarget`/`CurrentTarget` survive
until the id is reused.

<!-- b:472 -->
## `Interactable`

`interactable: (radius: ..)` on a `PrefabDef`, no collider needed. On the interact key within `radius` it emits
`entity.interacted:{id}`, or `entity.interact_blocked:{id}` instead (never both) when `requires_item` names an item the
player lacks; a blocked press still counts as a hit for miss detection. **`interactable_system` fires for every
in-range `Interactable` on one keypress, not just the nearest**, so place interactables apart (see the
`seal_door`/`merchant_01` note in `3rd_person_game_demo`'s `main.scene.ron`). It runs in `Update`
`.before(fsm_interpreter_system)`; `trigger_zone_system` runs in `FixedUpdate`.

<!-- b:524 -->
## Per-entity decay timers use `SetDespawnTimer`

Prefer `Action::SetDespawnTimer` (a `DespawnTimer` component on the target, ticked by `despawn_timer_system`) over
`EmitEventAfterDelay` + `Despawn` for **any** timer whose target's id might later be reused: a delayed event is global
and string-matched with no owner, so a stale timer from an older entity can despawn a newer one that reuses the id.

<!-- b:406 -->
## `RenderLayers` invariants for the party camera

The shared `ActiveCameraMode::Party` camera (`spawn_party_orbit_camera`, also the merged `dynamic` split camera) must
carry `all_ring_layers()` when `own_viewport_only` is true; with implicit layer 0 only it renders **zero** rings the
moment any ring restricts itself to a non-zero layer. Treat this as an invariant if the mechanism is extended.

<!-- b:352 -->
## Target-indicator ring colour

In a single-player scene a ring's colour comes from three-tier precedence: `PrefabDef.indicator_color`, then
`indicator_category` looked up in `TargetIndicatorDef.named_colors`, then `TargetIndicatorDef.color`. **With 2+ players
every ring uses the fixed `PLAYER_LABEL_COLORS` palette instead and the per-target precedence is overridden entirely.**
Two players on the same entity render two coincident rings, one per player colour, with **no deduplication: deliberate**
(a per-target colour could not show whose ring is whose). `target_indicator_system` runs in `Update` and keeps one
`TrackingTarget { target, owner }` per player.

<!-- b:433 -->
`pipeline_warmup_system`'s 4-frame `NoFrustumCulling` pass does not touch `RenderLayers`: a new `RenderLayers` consumer
must not assume warmup covers layer-restricted entities.

<!-- b:439 -->
Bevy's light visibility check intersects the **light's** `RenderLayers` (default layer 0) with the mesh's, so a mesh on
a non-zero layer only is dropped from every layer-0 light's shadow pass. The reserved-layer scheme is therefore only
clean for **unlit** cosmetics (rings are `unlit: true`); restricting a lit prefab to a player layer loses its shadows.

<!-- b:452.rule -->
`target_hud_update_system` is chained `.after(split_screen_viewport_system)` in `lib.rs`, like the corner-label update
system, so there is no stale frame across a `dynamic` split's merge/split transition.

<!-- b:1406 -->
## Per-rank duplication of world-space widgets

`world_label_screen_pos_system` (`lib.rs`) picks, per `WorldLabel`, the `WorldLabelRank`-th active camera whose
`logical_viewport_rect()` contains the projected point. Scene `world_labels:` and per-entity `label:` spawn
`MAX_SPLIT_PLAYERS` (4) rank siblings unconditionally. `stat_label`, the `Ascii`/`Pixel`/`Icon`/`Textured`
`world_stat_bar` styles and `ShowDamagePopup`/`ShowFloatingText` spawn one `WorldLabelRank` sibling per split slot **only
when the scene is split-screen** (`ActiveSplitScreen`/`DynamicSplitConfig` set) and exactly one rank-less entity
otherwise (they are rewritten every frame, so unconditional duplication is pure overhead). **Nameplate anchors are the
only `WorldLabel` widget that stays single-instance** (implicit rank 0: visible in at most one split viewport). A new
world-space widget spawn site must follow the same rank pattern.

<!-- b:1476 -->
`click_select_system` is viewport-aware: it filters active cameras to those whose `logical_viewport_rect()` contains
the cursor (ties by `camera_priority_key`); a click in a region no viewport covers does nothing.

<!-- b:1488 -->
**`nameplate_visibility_system` must read the `NameplateCameraDistance` that `world_label_screen_pos_system` stashes
on the anchor, never re-select a camera** (so the two cannot disagree). No stashed distance means off every active
viewport, which is out of range (hidden).

<!-- b:647 -->
## Animation pipeline rules

**Resume a paused clip before the next `transitions.play()`:** `AnimationTransitions::play` skips the fade-out for an
outgoing clip that `is_paused()`, so it would stay blended at full weight forever; `animation_playback_system` resumes
`last_played` first, and that is required, not cleanup.

<!-- b:658 -->
Seek with `ActiveAnimation::set_seek_time`, **not** `seek_to` (which replays every animation event between the old and
new time). Resolve a seek's clip duration from the live `AnimationGraph`/`AnimationNodeType::Clip`, not from a second
clip-name map that could desync from `node_indices`.

<!-- b:670 -->
`ActiveOverride.seek_fraction`/`.frozen` must stay **durable**, never consumed on first apply: the WASM-only
GLTF-hierarchy-respawn recovery path in `animation.rs` replays `transitions.play()` later, and a one-shot seek would
silently un-freeze a frozen pose (a corpse) on the web.

<!-- b:686 -->
## Dialogue system (`dialogue.rs`)

`DialoguePath(String)` is inserted by the scene loader on entities whose `PrefabDef.dialogue` is set;
`dialogue_tick_system` matches `entity.interacted:{id}` against them and fires `Action::StartDialogue`. It runs
`.after(button_system).after(interactable_system).before(fsm_interpreter_system)`. `ActiveDialogue` tracks the current
conversation and is cleared on `EndDialogue` and `LoadScene`. `advance_delay_secs` applies only when
`node.choices.is_empty()`. `{self}` in a choice's `do_actions` is replaced with `active.npc_id` by
`substitute_self_in_action()` before queueing. Events: `dialogue.started:{npc_id}`, `dialogue.ended:{dialogue_path}`.
`portrait` is reserved in `DialogueNodeDef` but not rendered; mark it unimplemented in authoring docs.

<!-- b:759 -->
## Deliberately left in `Update`

`targeting.rs`'s click-ray origin and tab-target distance sort (a one-tick error is far under any perceptual
threshold) and `particle_renderer.rs`'s billboard basis (uses only camera rotation) were reviewed and are deliberately
unconverted to the fixed tick. Do not re-investigate them as missed.

<!-- b:765 -->
On a display faster than 64Hz, motion-carrying entities (`motion.rs`) and physics bodies step visibly: a known,
accepted tradeoff, not a bug.

<!-- b:856.rule -->
## Ground detection hard rules (`player.rs`)

**The `jumps_used`/`jump_liftoff_y` reset must read `raw_grounded`, never the coyote-buffered
`LocomotionState.is_grounded`.** A buffer that smooths feel must not leak into a correctness check: with the buffered
value the reset fired on a flat-ground jump while `linvel.y` was still positive. The buffered value is fine for
`animation_resolver.rs`'s jump/land clip choice; the reset's grace/velocity/liftoff-height gate keeps `raw_grounded`.

<!-- b:779 -->
`jumps_used` is **not** reset on a `!was_grounded && is_grounded` edge (an edge-triggered reset can starve permanently if the
edge never fires); the landing animation request (`jump_exit`) still fires on that real edge. The reset is a separate
level-gated check re-evaluated every tick. A surface steeper than `CharacterController.max_walkable_slope_deg` (default 45;
`>= 90.0` restores the old proximity-only behaviour) never counts as grounded.

<!-- b:815 -->
`CharacterController.jump_air_grace` (a `FixedUpdate` tick countdown set when the jump fires) is derived analytically by
`jump_air_grace_ticks()` from the jump's own velocity, `GRAVITY` and the controller's `collider_radius`/`ground_cast_length`:
**never a separate hand-tuned constant**, which would drift from a project's authored values. While it is positive a grounded
reading is not even considered a landing.

<!-- b:950 -->
In the `ground_cast` re-query loop a hit may be excluded only if **both** conditions fail: it is not underfoot (contact at or
under `feet_pos.y + collider_radius * 0.5`) **and** it is not walkable (`is_walkable_contact`). "Not underfoot" alone imposes a
hidden 60-degree slope ceiling independent of `max_walkable_slope_deg` and ungrounds genuine floor contacts after a teleport or
`at_entity` placement; requiring both keeps the loop monotone and keeps the `>= 90.0` escape hatch exact.

<!-- b:912 -->
`can_jump`'s first-jump branch is `raw_grounded || (coyote_ticks_remaining > 0 && jumps_used == 0)`; the airborne
(double-jump) branch has no buffering and depends only on `raw_grounded`. **The coyote buffer may only ever unlock a
first jump (`jumps_used == 0`); gating the grounded branch on the buffered value without that qualifier makes
a double jump unreachable for the whole coyote window.**

<!-- b:929 -->
**The ground shape-cast must `.exclude_sensors()` and must `normalize_or_zero()` a hit normal before using it.** A
nearby prop's `trigger_zone` sensor can otherwise be swept like geometry: the cast starts embedded, its `toi == 0`
penetrating hit beats the real floor, and the radial normal reads as an unwalkable wall (the player plays the falling
animation on flat ground near any such prop). A penetrating hit's normal is not unit length, so a bare
`.dot(Vec3::Y).acos()` biases the angle toward 90 degrees. Any new physics query here follows the same rule. See
`tests/prop_ground_veto_tests.rs`.

<!-- b:206 -->
## Action-bar slot cost

A slot's cost check and its deferred deduct action's stat key are resolved **once** per firing slot (`resolve_cost_source` in
`action_bar.rs`) and reused for both, so the two can never disagree about which stat pool the cost hits.

<!-- b:1224 -->
**The player `Friction` coefficient is `PLAYER_IDLE_FRICTION` (0.15, `player.rs`) only while
`raw_grounded && !loco.moving`, and `0.0` otherwise**, re-synced every tick by `player_movement_system`; the initial
value is set in `spawn_player_entity_core`. Never apply it unconditionally: Rapier's Coulomb friction at a wall
contact resists the full tangent plane, crushing jump height by up to ~83% (`tests/wall_friction_tests.rs`). GLB and
primitive collider sizing deliberately stay different (capsule from `movement` config vs the prefab's `shape`).

<!-- b:1027 -->
## Terrain generation is async

Use `AsyncComputeTaskPool` and poll `Task` components; never block the main thread on terrain mesh generation.

<!-- b:1331 -->
## Split-screen camera prefabs

Only the **first** player-tagged scene entity's `camera.party`/`camera.split` is read, so `entities:` order matters in
local co-op; every split player still gets a real `ActiveCameraMode::Orbit` from their own `camera` block. A shared
mouse would drive every split camera, so **every split-screen player prefab must set all three of
`zoom_speed: 0.0`, `orbit_button: "None"` and `character_rotate_button: None`**: `character_rotate_button` is a
separate switch (default right-mouse) that otherwise spins every split player's character at once.

<!-- b:1359 -->
## Keyboard camera look

`orbit_button: "None"` disables only mouse orbit. Each player's `InputMap.look_left/right/up/down` are pre-resolved at
spawn onto `OrbitState` and applied in `camera_orbit_system` independent of the mouse gate; `CameraConfig.look_speed`
(rad/s, default 2.0) is the rate, deliberately not `orbit_speed` (a mouse-pixel multiplier). Pitch direction is pinned
to the mouse convention (`look_up` raises `pitch` toward `max_pitch`) by a regression test. `Party` has no equivalent
(no single owner).

<!-- b:1374 -->
`Action::CameraShake`'s camera query must stay `Or<(With<OrbitCameraMode>, With<PartyCameraMode>)>` and must **exclude**
`Fixed`/`FirstPerson`/`Flycam`: a flycam scene must keep the explicit "no orbit camera" `warn!`, because
`fly_camera_system` runs after the shake and rewrites `Transform::rotation` every frame.

<!-- b:1383 -->
## `SetCameraMode` and the camera-mode registry

The runtime switch is in `action_executor.rs`; `entity_spawner.rs::apply_camera_mode` is its switch-time analog of the
spawn-time arms in `spawn_active_camera_for_player` (some duplication, logged in `planning/claude_suggestions.md`).
- `AuthoredCameraMode` (starting mode, written once at spawn, never mutated) and `ActiveCameraMode` (live state) are
  **separate on purpose**: `SetCameraMode(mode: "default")` resolves against the former, everything else against
  `LoadedCameraModes` (the scene's registry, inserted in `scene_loader.rs`'s Replace branch only).
- `camera_blend_system` blends the rendered pose/FOV toward whatever the new mode's own system computed this frame, so
  it **must run after every per-mode camera system**: it is the last camera entry in `lib.rs`'s `Update` chain, right
  before `animation_playback_system`. Player input is not suppressed during a blend (a v2 simplification).
- `CameraModeOverride` is on a camera only while a registry-preset switch is active; `dynamic_split_screen_system`
  skips its automatic merge/split `is_active` toggle on any camera that has it.

<!-- b:1530.rule -->
## Gamepad consumers take `Option<&BoundGamepad>`

`input_translator_system`, `tab_targeting_system`, `interactable_system` and `action_bar_input_system` take
`Option<&BoundGamepad>`, **never a required `&BoundGamepad`** (a required one silently drops any test-built player
missing it from the whole query tuple, not just gamepad logic), and look the pad up with
`bound.and_then(|b| b.0).and_then(|e| gamepad_query.get(e).ok())`: no sorting and no positional lookup. `camera_orbit_system`
has no player entity in its query, so it resolves through a disjoint `Query<&BoundGamepad>` via `CameraTargets`. The
binding invariants themselves are in `../runtime/CLAUDE.md`.

<!-- b:1638 -->
## View-box clamp

`GameSceneV2.max_view_box` is read into `ActiveViewBox` on scene load (cleared on `LoadScene`).
`player_view_box_clamp_system` (`FixedUpdate`, after `player_movement_system`) clamps every `CharacterController`'s XZ
into the box and **zeroes the clamped axis's `Velocity.linvel`**; without that Rapier keeps integrating the outward
velocity and the player jitters against the edge.

<!-- b:1731 -->
## Particle pipeline warmup

Pool group entities are created lazily on first use, so `pipeline_warmup_system` cannot pre-warm them, and each new
material/blend combination (additive, blend, `PoolFlameMaterial`) compiles a pipeline synchronously on WASM (about
300-1000 ms). Fire one `SpawnEffect` per variant you use during scene load (at `y=-100`, next to `PreloadScene`/
`PreloadPrefab`; recipe in `docs/20_data_formats.md`, "Warming up particle pipelines"). **Those warmup effects are real particle allocations and consume `ParticleBudget`:** use low-count
effects, fire them on `scene.ready` before continuous emitters fill the pool, or size the budget for them.

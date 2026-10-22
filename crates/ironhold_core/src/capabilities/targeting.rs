use bevy::prelude::*;
use bevy::math::Isometry3d;
use bevy::window::PrimaryWindow;
use bevy::input::gamepad::Gamepad;
use crate::runtime::messages::GameEvent;
use crate::runtime::scene_manager::{SpawnId, PrefabKey, SpawnRegistry};
use crate::capabilities::action_bar::CurrentTarget;
use crate::capabilities::player::{BoundGamepad, CharacterController, PlayerIndex, PlayerTarget};
use crate::schema::player::InputMap;
use crate::GameVariables;
use crate::capabilities::inventory::LoadedInventoryUi;
use crate::capabilities::camera::{
    camera_priority_key, first_person_camera_system, ActiveCameraMode, CameraTargets, FirstPersonCameraMode,
    SplitViewportSlot,
};
use bevy_rapier3d::prelude::{QueryFilter, ReadRapierContext};

/// Pixel radius around the cursor within which a left-click selects an entity.
const SELECT_PIXEL_RADIUS: f32 = 70.0;
/// Vertical offset (metres) added to an entity's origin when projecting it to the
/// screen, so the click "aims" at body centre rather than the feet (entity origin).
const SELECT_AIM_HEIGHT: f32 = 1.0;

// ─── Components ───────────────────────────────────────────────────────────────

/// Marker: entity can be selected by left-clicking near it on screen.
/// Insert via `click_selectable: true` on `PrefabDef`.
#[derive(Component, Default)]
pub struct ClickSelectable;

/// Per-entity vertical offset (metres) for click-selection screen projection.
/// Inserted alongside `ClickSelectable`. Defaults to `SELECT_AIM_HEIGHT` (1.0).
/// Set via `select_aim_height` on `PrefabDef`.
#[derive(Component)]
pub struct SelectAimHeight(pub f32);

/// Marker: entity participates in Tab-cycle targeting (nearest-first).
/// Insert via `targetable: true` on `PrefabDef`.
#[derive(Component, Default)]
pub struct Targetable;

// ─── Plugin ───────────────────────────────────────────────────────────────────

pub struct TargetingPlugin;

impl Plugin for TargetingPlugin {
    fn build(&self, app: &mut App) {
        // Selection is done with screen-space projection (camera.world_to_viewport), NOT
        // mesh raycasting. Bevy's mesh picking raycasts bind-pose geometry, which does not
        // match the visible pose of skinned/animated GLB characters — clicks on an animated
        // orc would pass through to the ground. Projecting the entity origin to the screen
        // and measuring cursor distance is pose-independent and works for every prefab kind.
        //
        // The three targeting-mutation systems are explicitly chained in their documented
        // intent order (select/tab-cycle first, auto-clear last) and ordered before
        // `action_bar_input_system` — all four write/read `PlayerTarget`/`CurrentTarget` with
        // no prior ordering constraint between them, which let Bevy's scheduler serialize them
        // in whichever order thread availability happened to pick each run. On the exact frame a
        // targeted entity despawns, that let `action_bar_input_system` sometimes read a
        // just-cleared target and sometimes read the stale one depending purely on scheduling
        // luck — see `planning/backlog.md`'s "Same-frame targeting/action-bar race" bug entry.
        app.add_systems(Update, (
            (
                // Real pause: both are edge-triggered input systems; `target_auto_clear_system` must keep running.
                click_select_system.run_if(crate::runtime::sim_clock::sim_running),
                tab_targeting_system.run_if(crate::runtime::sim_clock::sim_running),
                target_auto_clear_system,
                // Crosshair targeting: the last writer of `PlayerTarget` in the chain, and ordered before the
                // first-person camera system, so it reads the pose the player actually saw last frame (one frame of
                // deterministic latency; `.after` it would form a cycle once the planned `CameraChainSet` is
                // moved after `GameplaySet::Execute`).
                aim_target_system
                    .run_if(crate::runtime::sim_clock::sim_running)
                    .before(first_person_camera_system),
            )
                .chain()
                // GameStreamSet::Targeting comes before GameStreamSet::ActionBar (the explicit replacement of the old
                // `.before(action_bar_input_system)` edge; the guarantee is now a declared set order, tested in
                // `tests/schedule_order_tests.rs`).
                .in_set(crate::runtime::schedule::GameStreamSet::Targeting),
            debug_selectables_system.run_if(resource_exists::<GizmoConfigStore>),
        ));
    }
}

// ─── Target display variables ───────────────────────────────────────────────

/// Writes the `GameVariables` that drive the target UI label, directly (no rule pipeline
/// dependency). Sets three keys so designers can bind whichever they need:
///   - `target_display` — `"<prefab> <id>"` (or just `<id>` if the prefab key is unknown)
///   - `target_name`    — the prefab catalog key (e.g. `"enemy_orc_melee"`)
///   - `target_id`      — the per-instance spawn id (e.g. `"orc_01"`)
pub(crate) fn write_target_vars(vars: &mut GameVariables, prefab: Option<&str>, id: &str) {
    let display = match prefab {
        Some(p) => format!("{} {}", p, id),
        None => id.to_string(),
    };
    vars.0.insert("target_display".to_string(), display);
    vars.0.insert("target_name".to_string(), prefab.unwrap_or(id).to_string());
    vars.0.insert("target_id".to_string(), id.to_string());
}

/// Blanks all target UI variables (target cleared, or 2+ players present — see
/// `apply_player_target`'s doc comment).
pub(crate) fn clear_target_vars(vars: &mut GameVariables) {
    vars.0.insert("target_display".to_string(), String::new());
    vars.0.insert("target_name".to_string(), String::new());
    vars.0.insert("target_id".to_string(), String::new());
}

/// A player entity carries either no `PlayerIndex` at all or `PlayerIndex(0)` — both mean "the
/// primary player". Since `player_model_source_unification.md` v1, a primitive-shaped player
/// spawned via the immediate scene-load path always gets a `PlayerIndex` (same as GLB players);
/// the "no `PlayerIndex` at all" case is now only reachable in practice via the v3-deferred
/// terrain/character-select paths, which don't spawn primitive players at all yet (see
/// `docs/dev/player-spawn-sites.md`). See
/// `planning/features/per_player_split_screen_targeting.md`. `pub(crate)` so
/// `action_executor.rs`'s `Action::SetTarget`/`ClearTarget` handlers can resolve the same primary
/// player these systems do, instead of writing `CurrentTarget` directly and leaving every
/// player's `PlayerTarget` untouched.
pub(crate) fn is_primary_player(player_index: Option<&PlayerIndex>) -> bool {
    player_index.map_or(true, |i| i.0 == 0)
}

/// Commits `id` as `player_target`'s new selection, mirrors it into the global `CurrentTarget`
/// resource when `is_primary` (so `{target}` substitution in `state_machine.ron`/
/// behaviors and the action bar's `{target}`-gated cost check keep resolving against the primary
/// player exactly as before per-player targets existed — this is Phase 1's documented scope
/// boundary, not a bug: a non-primary player's selection never reaches the shared action
/// pipeline), and syncs the legacy global `target_display`/`target_name`/`target_id`
/// `GameVariables`. Those vars go blank whenever 2+ players are present — there is no single
/// meaningful "the" target across independent players, so silently showing only the primary
/// player's value would be more confusing than a blank readout (use the per-viewport
/// `target_hud:` HUD instead for 2+ player scenes). Global pipeline events (`target.changed`/
/// `target.changed:{id}`) only fire for the primary player, for the same reason `CurrentTarget`
/// only mirrors the primary player.
pub(crate) fn apply_player_target(
    id: &str,
    prefab: Option<&str>,
    is_primary: bool,
    is_multiplayer: bool,
    player_target: &mut PlayerTarget,
    current_target: &mut CurrentTarget,
    game_vars: &mut GameVariables,
    game_events: &mut MessageWriter<GameEvent>,
) {
    player_target.0 = Some(id.to_string());
    if is_primary {
        current_target.0 = Some(id.to_string());
        game_events.write(GameEvent::Trigger(format!("target.changed:{}", id)));
        game_events.write(GameEvent::Trigger("target.changed".to_string()));
    }
    if is_multiplayer {
        clear_target_vars(game_vars);
    } else {
        write_target_vars(game_vars, prefab, id);
    }
    info!("Targeting: selected {:?} (prefab {:?}, primary: {})", id, prefab, is_primary);
}

/// Clears `player_target`'s selection, mirrors the clear into `CurrentTarget` when `is_primary`,
/// and blanks the legacy global vars — safe unconditionally: in a single-player scene this
/// correctly reflects "no target"; in a multiplayer scene the vars are already supposed to be
/// blank regardless (see `apply_player_target`).
pub(crate) fn clear_player_target(
    is_primary: bool,
    player_target: &mut PlayerTarget,
    current_target: &mut CurrentTarget,
    game_vars: &mut GameVariables,
    game_events: &mut MessageWriter<GameEvent>,
) {
    player_target.0 = None;
    if is_primary {
        current_target.0 = None;
        game_events.write(GameEvent::Trigger("target.cleared".to_string()));
    }
    clear_target_vars(game_vars);
}

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Left-click selects the `ClickSelectable` entity whose screen projection is nearest the
/// cursor (within `SELECT_PIXEL_RADIUS`). Clicking with nothing nearby clears the target.
///
/// Split-screen scenes have 2+ active `Camera3d` entities at once, each owning its own
/// on-screen viewport rect (`SplitViewportSlot`). The cursor is only ever meaningfully "in" the
/// viewport it's actually positioned over, so this picks the active camera whose own
/// `logical_viewport_rect()` contains the cursor, instead of an arbitrary first-active match —
/// otherwise a click in player 2's viewport could be evaluated against player 1's camera,
/// silently selecting the wrong entity. Ties (cursor inside 2+ viewport rects at once, not
/// expected for non-overlapping split layouts) break deterministically via `camera_priority_key`,
/// the same ordering `world_label_screen_pos_system` and `rebuild_pool_meshes_system` use.
///
/// The click is then attributed to *the player who owns that camera* — a split-screen camera's
/// `CameraTargets` points directly at its player entity. Cameras with no single "owning" player
/// (an empty `CameraTargets` — the default fallback camera in a scene with no player — **or** a
/// multi-target one, i.e. a shared party camera) fall back to the primary player instead — one
/// physical mouse can only ever act for one player per click regardless (an accepted, unavoidable
/// limitation, not something this system can fix — see
/// `planning/features/per_player_split_screen_targeting.md`). **Only a single-target
/// `CameraTargets` is used directly** — bug fix: a party camera's `CameraTargets` holds every
/// player, not zero, so naively taking `.first()` would attribute every click to whichever player
/// happened to spawn first, silently diverging from the primary-player convention whenever scene
/// entity order and `player_index` disagree (found by post-implementation review).
fn click_select_system(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(Entity, &Camera, &GlobalTransform, Option<&SplitViewportSlot>, Option<&CameraTargets>), With<Camera3d>>,
    selectables: Query<(Entity, &SpawnId, &GlobalTransform, Option<&PrefabKey>, Option<&SelectAimHeight>), With<ClickSelectable>>,
    visibility_q: Query<&Visibility>,
    mut player_targets: Query<(Entity, &mut PlayerTarget, Option<&PlayerIndex>)>,
    mut current_target: ResMut<CurrentTarget>,
    mut game_events: MessageWriter<GameEvent>,
    mut game_vars: ResMut<GameVariables>,
    ui_interactions: Query<&Interaction>,
    aim_cameras: Query<(&ActiveCameraMode, &CameraTargets), With<FirstPersonCameraMode>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    // Skip world targeting when ui_focus_system gave a UI node the click this frame.
    // Each panel root carries FocusPolicy::Block + Interaction, so any click inside
    // a panel rect registers Pressed on the panel root (or its child button). Clicks
    // outside open panels pass through normally and reach world targeting.
    if ui_interactions.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let Some((_, camera, cam_tf, _, targets)) = cameras
        .iter()
        .filter(|(_, camera, ..)| camera.is_active)
        .filter(|(_, camera, ..)| camera.logical_viewport_rect().is_some_and(|r| r.contains(cursor)))
        .min_by_key(|(entity, _, _, slot, _)| camera_priority_key(*entity, *slot))
    else { return };

    // Resolve which player this click acts for.
    let player_count = player_targets.iter().count();
    let single_owner = targets.filter(|t| t.0.len() == 1).and_then(|t| t.0.first().copied());
    let acting_player = single_owner.or_else(|| {
        player_targets.iter().find(|(_, _, idx)| is_primary_player(*idx)).map(|(e, _, _)| e)
    });
    let Some(acting_player) = acting_player else { return };
    // A player whose first-person camera has `aim_target` is targeted by the crosshair: a click is a shot,
    // not a selection (and a stray click-to-clear would fight the crosshair).
    if is_aim_driven(acting_player, &aim_cameras) {
        return;
    }
    let Ok((_, mut player_target, player_index)) = player_targets.get_mut(acting_player) else { return };
    let is_primary = is_primary_player(player_index);
    let is_multiplayer = player_count >= 2;

    // Find the nearest visible selectable to the cursor in screen space.
    let mut best: Option<(f32, String, Option<String>)> = None;
    for (entity, spawn_id, gt, prefab, aim_height) in &selectables {
        if visibility_q.get(entity).is_ok_and(|v| *v == Visibility::Hidden) {
            continue;
        }
        let height = aim_height.map_or(SELECT_AIM_HEIGHT, |h| h.0);
        let world = gt.translation() + Vec3::Y * height;
        let Ok(screen) = camera.world_to_viewport(cam_tf, world) else { continue };
        let dist = screen.distance(cursor);
        if dist <= SELECT_PIXEL_RADIUS && best.as_ref().map_or(true, |(bd, _, _)| dist < *bd) {
            best = Some((dist, spawn_id.0.clone(), prefab.map(|p| p.0.clone())));
        }
    }

    match best {
        Some((_, id, prefab)) => {
            game_events.write(GameEvent::Trigger(format!("target.clicked:{}", id)));
            apply_player_target(
                &id, prefab.as_deref(), is_primary, is_multiplayer,
                &mut player_target, &mut current_target, &mut game_vars, &mut game_events,
            );
        }
        None => {
            // Clicked empty space — clear this player's target.
            if player_target.0.is_some() {
                clear_player_target(is_primary, &mut player_target, &mut current_target, &mut game_vars, &mut game_events);
                info!("Targeting: cleared (clicked empty space)");
            }
        }
    }
}

/// Cycles each player's own `PlayerTarget` through nearby `Targetable` entities on that player's
/// own `target_next` key (from their `InputMap`). Hold Shift to cycle in reverse (nearest-last).
/// Processes every player independently in the same frame — player 1 and player 2 pressing their
/// own keys the same tick cycle their own, unrelated targets, with no shared state between them.
///
/// If two players' `InputMap.target_next` happen to bind to the same physical key (e.g. both left
/// at the "Tab" default on one shared keyboard), pressing it once cycles both players' targets
/// the same frame — expected, not a bug, exactly like every other per-player keybinding in this
/// engine (movement, jump, etc.) already behaves when two players share a keyboard.
pub fn tab_targeting_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut controllers: Query<(Entity, &CharacterController, &GlobalTransform, &mut PlayerTarget, Option<&PlayerIndex>, Option<&BoundGamepad>)>,
    gamepad_query: Query<&Gamepad>,
    targetable: Query<(Entity, &SpawnId, &GlobalTransform), With<Targetable>>,
    aim_cameras: Query<(&ActiveCameraMode, &CameraTargets), With<FirstPersonCameraMode>>,
    prefab_keys: Query<&PrefabKey>,
    visibility_q: Query<&Visibility>,
    mut current_target: ResMut<CurrentTarget>,
    mut game_events: MessageWriter<GameEvent>,
    mut game_vars: ResMut<GameVariables>,
    inventory_ui: Res<LoadedInventoryUi>,
) {
    if inventory_ui.panels_open > 0 { return; }
    let is_multiplayer = controllers.iter().count() >= 2;

    for (player_entity, controller, player_gt, mut player_target, player_index, bound) in &mut controllers {
        // Crosshair-targeted players do not cycle (see `aim_target_system`).
        if is_aim_driven(player_entity, &aim_cameras) {
            continue;
        }
        // `None` = unbound (keyboard cycling off, gamepad unaffected); an authored but
        // unrecognised name keeps the historical fallback to Tab.
        let tab_key = controller.inputs.target_next.as_deref()
            .map(|name| InputMap::parse_key(name).unwrap_or(KeyCode::Tab));
        let gamepad = bound.and_then(|b| b.0).and_then(|e| gamepad_query.get(e).ok());
        let gamepad_target_next = controller.inputs.gamepad_button("target_next");
        let gamepad_pressed = gamepad.zip(gamepad_target_next)
            .map(|(gp, btn)| gp.just_pressed(btn))
            .unwrap_or(false);
        if !tab_key.is_some_and(|k| keys.just_pressed(k)) && !gamepad_pressed {
            continue;
        }

        let reverse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let player_pos = player_gt.translation();
        let range = controller.inputs.target_range;

        let mut candidates: Vec<(Entity, String, f32)> = targetable
            .iter()
            .filter_map(|(entity, spawn_id, gt)| {
                if visibility_q.get(entity).is_ok_and(|v| *v == Visibility::Hidden) {
                    return None;
                }
                let dist = gt.translation().distance(player_pos);
                if dist <= range { Some((entity, spawn_id.0.clone(), dist)) } else { None }
            })
            .collect();

        if candidates.is_empty() {
            debug!("Tab targeting: no targetable entities within {:.1} units of the player", range);
            continue;
        }

        candidates.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        if reverse { candidates.reverse(); }

        let next_idx = match &player_target.0 {
            Some(cur) => {
                let idx = candidates.iter().position(|(_, id, _)| id == cur);
                idx.map_or(0, |i| (i + 1) % candidates.len())
            }
            None => 0,
        };
        let (next_entity, next_id, _) = &candidates[next_idx];
        let next_id = next_id.clone();
        let prefab = prefab_keys.get(*next_entity).ok().map(|p| p.0.clone());

        let is_primary = is_primary_player(player_index);
        apply_player_target(
            &next_id, prefab.as_deref(), is_primary, is_multiplayer,
            &mut player_target, &mut current_target, &mut game_vars, &mut game_events,
        );
    }
}

/// Whether `player` is the first target of a first-person camera that has `aim_target`: the crosshair, not the
/// mouse cursor or the Tab key, then decides its `PlayerTarget`.
fn is_aim_driven(
    player: Entity,
    cameras: &Query<(&ActiveCameraMode, &CameraTargets), With<FirstPersonCameraMode>>,
) -> bool {
    cameras.iter().any(|(mode, targets)| {
        matches!(mode, ActiveCameraMode::FirstPerson(fp) if fp.aim_target.is_some())
            && targets.0.first() == Some(&player)
    })
}

/// `FirstPersonCameraDef.aim_target`: sets each aiming player's `PlayerTarget` from the **centre of the screen**.
///
/// Per aiming camera: among the `Targetable` entities within `range` metres of the camera and `radius_deg` of the
/// view axis, take the one nearest the screen centre (the same screen-space projection and `SelectAimHeight` aim
/// point `click_select_system` uses, pose-independent so it works on animated GLBs); then, with
/// `require_line_of_sight`, only accept it if a single Rapier ray from the camera reaches it (sensors and the
/// player's own body are ignored), falling back to the next nearest. Ties are broken by `SpawnId`, never by
/// `Entity` order. The result goes through the same `apply_player_target`/`clear_player_target` as click and Tab
/// selection, so `{target}`, `target.changed`, the `target_*` variables and the action bar's `{target}` gating
/// need no change, and a frame that changes nothing writes nothing.
///
/// The camera pose is the previous frame's final `GlobalTransform` (this system runs before
/// `first_person_camera_system`), which is exactly the picture the player is looking at.
pub fn aim_target_system(
    cameras: Query<
        (&Camera, &GlobalTransform, &ActiveCameraMode, &CameraTargets, Option<&Projection>),
        (With<FirstPersonCameraMode>, With<Camera3d>),
    >,
    targetables: Query<(Entity, &SpawnId, &GlobalTransform, Option<&PrefabKey>, Option<&SelectAimHeight>), With<Targetable>>,
    visibility_q: Query<&Visibility>,
    parents: Query<&ChildOf>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut player_targets: Query<(&mut PlayerTarget, Option<&PlayerIndex>)>,
    mut current_target: ResMut<CurrentTarget>,
    mut game_events: MessageWriter<GameEvent>,
    mut game_vars: ResMut<GameVariables>,
    rapier_context: Option<ReadRapierContext>,
) {
    // Most scenes have no aiming camera: do no setup (physics context, player count) for them.
    if !cameras.iter().any(|(_, _, mode, ..)| matches!(mode, ActiveCameraMode::FirstPerson(fp) if fp.aim_target.is_some())) {
        return;
    }
    let rapier = rapier_context.as_ref().and_then(|rc| rc.single().ok());
    let is_multiplayer = player_targets.iter().count() >= 2;

    for (camera, cam_tf, mode, targets, projection) in &cameras {
        let ActiveCameraMode::FirstPerson(fp) = mode else { continue };
        let Some(aim) = fp.aim_target.as_ref() else { continue };
        let Some(&owner) = targets.0.first() else { continue };
        // The camera's own logical viewport; before the camera has one (the first frame, a headless test app)
        // a full-window first-person camera falls back to the window size.
        let Some(size) = camera
            .logical_viewport_size()
            .or_else(|| windows.single().ok().map(|w| Vec2::new(w.width(), w.height())))
        else { continue };
        let fov = match projection {
            Some(Projection::Perspective(p)) => p.fov,
            _ => 90.0_f32.to_radians(),
        };
        // Angular catch radius -> pixels on this viewport: tan(r) / tan(fov / 2) of the half height.
        let radius_px = aim.radius_deg.to_radians().tan() / (fov * 0.5).tan().max(f32::EPSILON) * (size.y * 0.5);
        let center = size * 0.5;
        let cam_pos = cam_tf.translation();
        let projector = ViewProjector::new(cam_tf, fov, size); // one matrix inverse per camera, not per target

        // Candidates inside the catch circle and range, nearest the centre first (ties by SpawnId).
        let mut candidates: Vec<(f32, &str, Entity, Vec3, Option<&str>)> = Vec::new();
        for (entity, spawn_id, gt, prefab, aim_height) in &targetables {
            if entity == owner || visibility_q.get(entity).is_ok_and(|v| *v == Visibility::Hidden) {
                continue;
            }
            let height = aim_height.map_or(SELECT_AIM_HEIGHT, |h| h.0);
            let world = gt.translation() + Vec3::Y * height;
            if world.distance(cam_pos) > aim.range {
                continue;
            }
            let Some(screen) = projector.project(world) else { continue };
            let d = screen.distance(center);
            if d <= radius_px {
                candidates.push((d, spawn_id.0.as_str(), entity, world, prefab.map(|p| p.0.as_str())));
            }
        }
        candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.1.cmp(b.1)));

        let chosen = candidates.iter().find(|(_, _, entity, world, _)| {
            if !aim.require_line_of_sight {
                return true;
            }
            let Some(ctx) = rapier.as_ref() else { return true }; // no physics (headless): nothing to block
            let to = *world - cam_pos;
            let dist = to.length();
            if dist < 0.01 {
                return true;
            }
            let hit = ctx.cast_ray(
                cam_pos,
                (to / dist).into(),
                dist,
                true,
                QueryFilter::new().exclude_rigid_body(owner).exclude_sensors(),
            );
            match hit {
                None => true, // the target has no collider on the way: nothing in front of it
                Some((hit_entity, _)) => is_self_or_descendant(hit_entity, *entity, &parents),
            }
        });

        let Ok((mut player_target, player_index)) = player_targets.get_mut(owner) else { continue };
        let is_primary = is_primary_player(player_index);
        match chosen {
            Some((_, id, _, _, prefab)) => {
                if player_target.0.as_deref() != Some(*id) {
                    apply_player_target(
                        id, *prefab, is_primary, is_multiplayer,
                        &mut player_target, &mut current_target, &mut game_vars, &mut game_events,
                    );
                }
            }
            None => {
                if player_target.0.is_some() {
                    clear_player_target(is_primary, &mut player_target, &mut current_target, &mut game_vars, &mut game_events);
                }
            }
        }
    }
}

/// Pinhole projection of a world point onto a `size`-pixel viewport seen through a perspective camera with the
/// given vertical field of view: pixels from the top-left, `None` when the point is behind the camera. Same result
/// as `Camera::world_to_viewport` for a camera whose aspect ratio follows its viewport, but needs no render target,
/// so the aim system also works in a headless test app.
pub fn project_to_viewport(view: &GlobalTransform, vertical_fov: f32, size: Vec2, world: Vec3) -> Option<Vec2> {
    ViewProjector::new(view, vertical_fov, size).project(world)
}

/// [`project_to_viewport`] with the camera's inverse transform computed once, for projecting many points through
/// the same camera in one frame (the aim system projects every targetable).
pub struct ViewProjector {
    inverse: bevy::math::Affine3A,
    tan_half: f32,
    aspect: f32,
    size: Vec2,
}

impl ViewProjector {
    pub fn new(view: &GlobalTransform, vertical_fov: f32, size: Vec2) -> Self {
        Self {
            inverse: view.affine().inverse(),
            tan_half: (vertical_fov * 0.5).tan(),
            aspect: size.x / size.y.max(f32::EPSILON),
            size,
        }
    }

    /// Pixels from the top-left of the viewport, `None` when the point is behind the camera.
    pub fn project(&self, world: Vec3) -> Option<Vec2> {
        let local = self.inverse.transform_point3(world);
        if local.z >= -f32::EPSILON {
            return None;
        }
        let ndc_x = local.x / (-local.z * self.tan_half * self.aspect);
        let ndc_y = local.y / (-local.z * self.tan_half);
        Some(Vec2::new((ndc_x + 1.0) * 0.5 * self.size.x, (1.0 - ndc_y) * 0.5 * self.size.y))
    }
}

/// `entity` is `target` itself or one of its descendants (a collider may sit on a child of the targetable entity).
fn is_self_or_descendant(entity: Entity, target: Entity, parents: &Query<&ChildOf>) -> bool {
    let mut current = entity;
    for _ in 0..16 {
        if current == target {
            return true;
        }
        match parents.get(current) {
            Ok(child_of) => current = child_of.parent(),
            Err(_) => return false,
        }
    }
    false
}

/// Draws a yellow wireframe sphere at each `ClickSelectable` entity's aim point
/// (world origin + `SELECT_AIM_HEIGHT`) when the `"debug_target_hitboxes"` GameVariable is `"true"`.
/// Toggle via `SetVariable("debug_target_hitboxes", "true"/"false")` in RON rules — no recompile needed.
fn debug_selectables_system(
    game_vars: Res<GameVariables>,
    selectables: Query<(&GlobalTransform, Option<&SelectAimHeight>), With<ClickSelectable>>,
    mut gizmos: Gizmos,
) {
    if game_vars.0.get("debug_target_hitboxes").map(String::as_str) != Some("true") {
        return;
    }
    for (gt, aim_height) in &selectables {
        let height = aim_height.map_or(SELECT_AIM_HEIGHT, |h| h.0);
        let aim = gt.translation() + Vec3::Y * height;
        gizmos.sphere(Isometry3d::from_translation(aim), 0.5, Color::srgba(1.0, 1.0, 0.0, 0.85));
    }
}

/// Clears each player's `PlayerTarget` independently when their targeted entity becomes hidden
/// (e.g. dead) or is genuinely despawned (e.g. a corpse swap or decay removing the entity
/// entirely) — one player's target being cleared does not touch any other player's target.
/// Prevents the action bar from firing at invisible/nonexistent enemies and keeps the target UI
/// clean.
///
/// The despawned case matters specifically for monster_corpse_loot.md v2: a targeted monster's
/// `Despawn("{self}")` on death removes it from `SpawnRegistry` outright rather than merely
/// hiding it, so `registry.entities.get(&target_id)` returning `None` must be treated the same
/// as "hidden," not skipped — otherwise the stale target silently survives until the same id is
/// reused by that slot's next respawn (up to 60s later), at which point the player appears to
/// have re-targeted an entity they never clicked.
pub fn target_auto_clear_system(
    mut controllers: Query<(&mut PlayerTarget, Option<&PlayerIndex>), With<CharacterController>>,
    mut current_target: ResMut<CurrentTarget>,
    mut game_vars: ResMut<GameVariables>,
    mut game_events: MessageWriter<GameEvent>,
    registry: Res<SpawnRegistry>,
    visibility_q: Query<&Visibility>,
) {
    for (mut player_target, player_index) in &mut controllers {
        let Some(target_id) = player_target.0.clone() else { continue };
        let should_clear = match registry.entities.get(&target_id) {
            Some(&entity) => visibility_q.get(entity).is_ok_and(|vis| *vis == Visibility::Hidden),
            None => true,
        };
        if should_clear {
            let is_primary = is_primary_player(player_index);
            clear_player_target(is_primary, &mut player_target, &mut current_target, &mut game_vars, &mut game_events);
            info!("Targeting: auto-cleared '{}' (entity hidden or despawned)", target_id);
        }
    }
}

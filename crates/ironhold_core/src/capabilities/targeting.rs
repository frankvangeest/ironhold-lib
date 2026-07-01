use bevy::prelude::*;
use bevy::math::Isometry3d;
use bevy::window::PrimaryWindow;
use crate::runtime::messages::GameEvent;
use crate::runtime::scene_manager::{SpawnId, PrefabKey, SpawnRegistry};
use crate::capabilities::action_bar::CurrentTarget;
use crate::capabilities::player::CharacterController;
use crate::schema::player::InputMap;
use crate::GameVariables;
use crate::capabilities::inventory::LoadedInventoryUi;

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
        app.add_systems(Update, (
            click_select_system,
            tab_targeting_system,
            target_auto_clear_system,
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

/// Blanks all target UI variables (target cleared).
pub(crate) fn clear_target_vars(vars: &mut GameVariables) {
    vars.0.insert("target_display".to_string(), String::new());
    vars.0.insert("target_name".to_string(), String::new());
    vars.0.insert("target_id".to_string(), String::new());
}

/// Commits a new current target: updates the resource + UI vars and emits pipeline events.
fn apply_target(
    id: &str,
    prefab: Option<&str>,
    current_target: &mut CurrentTarget,
    game_vars: &mut GameVariables,
    game_events: &mut MessageWriter<GameEvent>,
) {
    current_target.0 = Some(id.to_string());
    write_target_vars(game_vars, prefab, id);
    game_events.write(GameEvent::Trigger(format!("target.changed:{}", id)));
    game_events.write(GameEvent::Trigger("target.changed".to_string()));
    info!("Targeting: selected {:?} (prefab {:?})", id, prefab);
}

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Left-click selects the `ClickSelectable` entity whose screen projection is nearest the
/// cursor (within `SELECT_PIXEL_RADIUS`). Clicking with nothing nearby clears the target.
fn click_select_system(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    selectables: Query<(Entity, &SpawnId, &GlobalTransform, Option<&PrefabKey>, Option<&SelectAimHeight>), With<ClickSelectable>>,
    visibility_q: Query<&Visibility>,
    mut current_target: ResMut<CurrentTarget>,
    mut game_events: MessageWriter<GameEvent>,
    mut game_vars: ResMut<GameVariables>,
    ui_interactions: Query<&Interaction>,
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
    let Some((camera, cam_tf)) = cameras.iter().find(|(c, _)| c.is_active) else { return };

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
            apply_target(&id, prefab.as_deref(), &mut current_target, &mut game_vars, &mut game_events);
        }
        None => {
            // Clicked empty space — clear any current target.
            if current_target.0.is_some() {
                current_target.0 = None;
                clear_target_vars(&mut game_vars);
                game_events.write(GameEvent::Trigger("target.cleared".to_string()));
                info!("Targeting: cleared (clicked empty space)");
            }
        }
    }
}

/// Cycles `CurrentTarget` through nearby `Targetable` entities on the configured key.
/// Hold Shift to cycle in reverse (nearest-last).
pub fn tab_targeting_system(
    keys: Res<ButtonInput<KeyCode>>,
    controllers: Query<(&CharacterController, &GlobalTransform)>,
    targetable: Query<(Entity, &SpawnId, &GlobalTransform), With<Targetable>>,
    prefab_keys: Query<&PrefabKey>,
    visibility_q: Query<&Visibility>,
    mut current_target: ResMut<CurrentTarget>,
    mut game_events: MessageWriter<GameEvent>,
    mut game_vars: ResMut<GameVariables>,
    inventory_ui: Res<LoadedInventoryUi>,
) {
    if inventory_ui.panels_open > 0 { return; }
    let Some((controller, player_gt)) = controllers.iter().next() else { return };

    let tab_key = InputMap::parse_key(&controller.inputs.target_next)
        .unwrap_or(KeyCode::Tab);
    if !keys.just_pressed(tab_key) {
        return;
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
        return;
    }

    candidates.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
    if reverse { candidates.reverse(); }

    let next_idx = match &current_target.0 {
        Some(cur) => {
            let idx = candidates.iter().position(|(_, id, _)| id == cur);
            idx.map_or(0, |i| (i + 1) % candidates.len())
        }
        None => 0,
    };
    let (next_entity, next_id, _) = &candidates[next_idx];
    let next_id = next_id.clone();
    let prefab = prefab_keys.get(*next_entity).ok().map(|p| p.0.clone());

    apply_target(&next_id, prefab.as_deref(), &mut current_target, &mut game_vars, &mut game_events);
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

/// Clears `CurrentTarget` when the targeted entity becomes hidden (e.g. dead/despawned).
/// Prevents the action bar from firing at invisible enemies and keeps the target UI clean.
pub fn target_auto_clear_system(
    mut current_target: ResMut<CurrentTarget>,
    mut game_vars: ResMut<GameVariables>,
    mut game_events: MessageWriter<GameEvent>,
    registry: Res<SpawnRegistry>,
    visibility_q: Query<&Visibility>,
) {
    let Some(target_id) = current_target.0.clone() else { return };
    let Some(&entity) = registry.entities.get(&target_id) else { return };
    let Ok(vis) = visibility_q.get(entity) else { return };
    if *vis == Visibility::Hidden {
        current_target.0 = None;
        clear_target_vars(&mut game_vars);
        game_events.write(GameEvent::Trigger("target.cleared".to_string()));
        info!("Targeting: auto-cleared '{}' (entity hidden)", target_id);
    }
}

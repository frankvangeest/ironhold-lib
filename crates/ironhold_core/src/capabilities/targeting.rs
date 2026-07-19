use bevy::prelude::*;
use bevy::math::Isometry3d;
use bevy::window::PrimaryWindow;
use crate::runtime::messages::GameEvent;
use crate::runtime::scene_manager::{SpawnId, PrefabKey, SpawnRegistry};
use crate::capabilities::action_bar::CurrentTarget;
use crate::capabilities::player::{CharacterController, PlayerIndex, PlayerTarget};
use crate::schema::player::InputMap;
use crate::GameVariables;
use crate::capabilities::inventory::LoadedInventoryUi;
use crate::capabilities::camera::{camera_priority_key, OrbitCamera, SplitViewportSlot};

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
/// `crates/ironhold_core/src/CLAUDE.md`'s "player-construction" section). See
/// `planning/features/per_player_split_screen_targeting.md`. `pub(crate)` so
/// `action_executor.rs`'s `Action::SetTarget`/`ClearTarget` handlers can resolve the same primary
/// player these systems do, instead of writing `CurrentTarget` directly and leaving every
/// player's `PlayerTarget` untouched.
pub(crate) fn is_primary_player(player_index: Option<&PlayerIndex>) -> bool {
    player_index.map_or(true, |i| i.0 == 0)
}

/// Commits `id` as `player_target`'s new selection, mirrors it into the global `CurrentTarget`
/// resource when `is_primary` (so `{target}` substitution in `rules.ron`/`state_machine.ron`/
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
/// `OrbitCamera.target` points directly at its player entity. Cameras with no `OrbitCamera` at
/// all (a shared `PartyOrbitCamera`, or the default fallback camera in a scene with no player)
/// have no single "owning" player, so the click falls back to the primary player instead — one
/// physical mouse can only ever act for one player per click regardless (an accepted, unavoidable
/// limitation, not something this system can fix — see
/// `planning/features/per_player_split_screen_targeting.md`).
fn click_select_system(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(Entity, &Camera, &GlobalTransform, Option<&SplitViewportSlot>, Option<&OrbitCamera>), With<Camera3d>>,
    selectables: Query<(Entity, &SpawnId, &GlobalTransform, Option<&PrefabKey>, Option<&SelectAimHeight>), With<ClickSelectable>>,
    visibility_q: Query<&Visibility>,
    mut player_targets: Query<(Entity, &mut PlayerTarget, Option<&PlayerIndex>)>,
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
    let Some((_, camera, cam_tf, _, orbit)) = cameras
        .iter()
        .filter(|(_, camera, ..)| camera.is_active)
        .filter(|(_, camera, ..)| camera.logical_viewport_rect().is_some_and(|r| r.contains(cursor)))
        .min_by_key(|(entity, _, _, slot, _)| camera_priority_key(*entity, *slot))
    else { return };

    // Resolve which player this click acts for.
    let player_count = player_targets.iter().count();
    let acting_player = orbit.map(|o| o.target).or_else(|| {
        player_targets.iter().find(|(_, _, idx)| is_primary_player(*idx)).map(|(e, _, _)| e)
    });
    let Some(acting_player) = acting_player else { return };
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
    mut controllers: Query<(&CharacterController, &GlobalTransform, &mut PlayerTarget, Option<&PlayerIndex>)>,
    targetable: Query<(Entity, &SpawnId, &GlobalTransform), With<Targetable>>,
    prefab_keys: Query<&PrefabKey>,
    visibility_q: Query<&Visibility>,
    mut current_target: ResMut<CurrentTarget>,
    mut game_events: MessageWriter<GameEvent>,
    mut game_vars: ResMut<GameVariables>,
    inventory_ui: Res<LoadedInventoryUi>,
) {
    if inventory_ui.panels_open > 0 { return; }
    let is_multiplayer = controllers.iter().count() >= 2;

    for (controller, player_gt, mut player_target, player_index) in &mut controllers {
        let tab_key = InputMap::parse_key(&controller.inputs.target_next)
            .unwrap_or(KeyCode::Tab);
        if !keys.just_pressed(tab_key) {
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
/// (e.g. dead/despawned) — one player's target being hidden does not touch any other player's
/// target. Prevents the action bar from firing at invisible enemies and keeps the target UI
/// clean.
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
        let Some(&entity) = registry.entities.get(&target_id) else { continue };
        let Ok(vis) = visibility_q.get(entity) else { continue };
        if *vis == Visibility::Hidden {
            let is_primary = is_primary_player(player_index);
            clear_player_target(is_primary, &mut player_target, &mut current_target, &mut game_vars, &mut game_events);
            info!("Targeting: auto-cleared '{}' (entity hidden)", target_id);
        }
    }
}

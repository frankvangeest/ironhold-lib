use bevy::prelude::*;
use bevy::input::gamepad::Gamepad;
use crate::capabilities::player::{BoundGamepad, CharacterController};
use crate::capabilities::inventory::LoadedInventoryUi;
use crate::runtime::messages::*;
use crate::runtime::scene_manager::SpawnId;

/// Marks an entity as player-interactable.
///
/// When the player is within `radius` metres and presses the interact key (configured via
/// `inputs.interact` in the player prefab, default: `"KeyF"`), the system emits:
///   `GameEvent::Trigger("entity.interacted:{spawn_id}")`
///
/// The response is configured in RON — either in `rules.ron`, `state_machine.ron`,
/// or the entity's own `.behavior.ron` file.
#[derive(Component)]
pub struct Interactable {
    /// Player must be closer than this (metres) to trigger interaction.
    pub radius: f32,
    /// Optional hint shown near the entity when the player enters range.
    /// Not yet rendered — reserved for a future UI pass.
    pub hint_text: Option<String>,
}

/// Checks each frame whether each player is within range of any `Interactable` entity
/// and that player's own interact key/button was just pressed. The interact key is read from
/// each player's own `InputMap.interact` field (default: `"KeyF"`); the gamepad equivalent from
/// `InputMap.gamepad_interact` (default: `"West"`). Runs in `Update`.
///
/// Per-player loop (mirrors `tab_targeting_system`'s shape) — this system previously used
/// `player_query.single()`, which fails and early-returns for *every* player the moment a scene
/// has 2+ `CharacterController`s, so interact silently did nothing for anyone in any
/// local-coop/split-screen scene. Found during `gamepad_controller_input.md`'s plan review
/// (system-architect, 2026-07-19); fixed independently of that feature since the bug predates
/// gamepad input entirely. The gamepad check below was added by that same feature once this fix
/// landed — folds into the per-player loop exactly like `tab_targeting_system`'s own gamepad
/// fold, so gamepad-interact works in local co-op too, not just single-player.
pub fn interactable_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    player_query: Query<(&Transform, &CharacterController, Option<&BoundGamepad>)>,
    gamepad_query: Query<&Gamepad>,
    interactables: Query<(&Transform, &SpawnId, &Interactable)>,
    mut game_events: MessageWriter<GameEvent>,
    inventory_ui: Res<LoadedInventoryUi>,
) {
    if inventory_ui.panels_open > 0 { return; }

    for (player_transform, controller, bound) in &player_query {
        let keyboard_pressed = controller.inputs.key("interact")
            .map(|k| keyboard_input.just_pressed(k))
            .unwrap_or(false);
        let gamepad = bound.and_then(|b| b.0).and_then(|e| gamepad_query.get(e).ok());
        let gamepad_interact = controller.inputs.gamepad_button("interact");
        let gamepad_pressed = gamepad.zip(gamepad_interact)
            .map(|(gp, btn)| gp.just_pressed(btn))
            .unwrap_or(false);
        if !keyboard_pressed && !gamepad_pressed {
            continue;
        }

        let mut hit_any = false;
        for (transform, spawn_id, interactable) in &interactables {
            let dist = player_transform.translation.distance(transform.translation);
            if dist <= interactable.radius {
                info!("Interacted with: {}", spawn_id.0);
                game_events.write(GameEvent::Trigger(format!(
                    "entity.interacted:{}",
                    spawn_id.0
                )));
                hit_any = true;
            }
        }
        if !hit_any {
            game_events.write(GameEvent::Trigger("player.attack_missed".to_string()));
        }
    }
}

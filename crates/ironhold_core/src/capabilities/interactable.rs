use bevy::prelude::*;
use crate::capabilities::player::CharacterController;
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

/// Checks each frame whether the player is within range of any `Interactable` entity
/// and the interact key was just pressed. The interact key is read from the player's
/// `InputMap.interact` field (default: `"KeyF"`). Runs in `Update`.
pub fn interactable_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    player_query: Query<(&Transform, &CharacterController)>,
    interactables: Query<(&Transform, &SpawnId, &Interactable)>,
    mut game_events: MessageWriter<GameEvent>,
) {
    let Ok((player_transform, controller)) = player_query.single() else { return };
    let Some(interact_key) = controller.inputs.key("interact") else { return };
    if !keyboard_input.just_pressed(interact_key) {
        return;
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

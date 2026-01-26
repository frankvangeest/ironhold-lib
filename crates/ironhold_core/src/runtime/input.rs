use bevy::prelude::*;
use crate::runtime::messages::*;
use crate::capabilities::player::CharacterController;

pub fn input_translator_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    query: Query<(Entity, &CharacterController)>,
    mut input_events: MessageWriter<InputActionMessage>,
) {
    for (entity, controller) in &query {
        let mut move_vec = Vec2::ZERO;
        
        if let Some(key) = controller.inputs.key("forward") {
            if keyboard_input.pressed(key) { move_vec.y += 1.0; }
        }
        if let Some(key) = controller.inputs.key("backward") {
            if keyboard_input.pressed(key) { move_vec.y -= 1.0; }
        }
        if let Some(key) = controller.inputs.key("right") {
            if keyboard_input.pressed(key) { move_vec.x += 1.0; }
        }
        if let Some(key) = controller.inputs.key("left") {
            if keyboard_input.pressed(key) { move_vec.x -= 1.0; }
        }
        
        // info!("move_vec = {:?}", move_vec);
        if move_vec != Vec2::ZERO {
            input_events.write(InputActionMessage {
                entity,
                action: InputAction::Move(move_vec.normalize()),
            });
        }

        let mut turn = 0.0;
        if let Some(key) = controller.inputs.key("left") {
            if keyboard_input.pressed(key) { turn += 1.0; }
        }
        if let Some(key) = controller.inputs.key("right") {
            if keyboard_input.pressed(key) { turn -= 1.0; }
        }

        if turn != 0.0 {
            input_events.write(InputActionMessage {
                entity,
                action: InputAction::Turn(turn),
            });
        }

        if let Some(key) = controller.inputs.key("jump") {
            if keyboard_input.just_pressed(key) {
                input_events.write(InputActionMessage {
                    entity,
                    action: InputAction::Jump(true),
                });
            }
        }

        if let Some(key) = controller.inputs.key("run") {
            if keyboard_input.just_pressed(key) {
                input_events.write(InputActionMessage {
                    entity,
                    action: InputAction::Run(true),
                });
            }
        }
    }
}

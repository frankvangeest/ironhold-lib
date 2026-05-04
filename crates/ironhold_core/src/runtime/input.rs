use bevy::prelude::*;
use crate::runtime::messages::*;
use crate::runtime::scene_manager::LoadedKeyBindings;
use crate::capabilities::player::CharacterController;
use crate::schema::AppState;
use crate::schema::player::InputMap;

/// Translates global key presses into UI messages using the project's `global_key_bindings`.
/// Only fires in `InGame` state. Runs in `Update` (not `FixedUpdate`) so it
/// responds every rendered frame regardless of physics tick rate.
pub fn global_input_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    state: Res<State<AppState>>,
    key_bindings: Res<LoadedKeyBindings>,
    mut ui_events: MessageWriter<UiEvent>,
) {
    if *state.get() != AppState::InGame { return; }

    for (key_name, trigger) in &key_bindings.0 {
        if let Some(key_code) = InputMap::parse_key(key_name) {
            if keyboard_input.just_pressed(key_code) {
                ui_events.write(UiEvent::ButtonPressed(trigger.clone()));
            }
        } else {
            // Unknown key name — silently skip. A warning is logged at project load time
            // in check_project_loaded so the designer gets early feedback without spamming.
        }
    }
}

pub fn input_translator_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    query: Query<(Entity, &CharacterController)>,
    mut input_events: MessageWriter<InputActionMessage>,
    #[cfg(feature = "inspector")]
    inspector_enabled: Option<Res<crate::inspector::InspectorEnabled>>,
) {
    #[cfg(feature = "inspector")]
    if let Some(enabled) = inspector_enabled {
        if enabled.0 {
            return;
        }
    }

    for (entity, controller) in &query {
        let strafe_mode = controller.inputs.strafe_mouse_button
            .as_deref()
            .and_then(InputMap::parse_mouse_button)
            .map(|btn| mouse_input.pressed(btn))
            .unwrap_or(false);
        let mut move_vec = Vec2::ZERO;

        if let Some(key) = controller.inputs.key("forward") {
            if keyboard_input.pressed(key) { move_vec.y += 1.0; }
        }
        if let Some(key) = controller.inputs.key("backward") {
            if keyboard_input.pressed(key) { move_vec.y -= 1.0; }
        }
        // A/D strafe only when left mouse is held.
        if strafe_mode {
            if let Some(key) = controller.inputs.key("right") {
                if keyboard_input.pressed(key) { move_vec.x += 1.0; }
            }
            if let Some(key) = controller.inputs.key("left") {
                if keyboard_input.pressed(key) { move_vec.x -= 1.0; }
            }
        }

        // info!("move_vec = {:?}", move_vec);
        if move_vec != Vec2::ZERO {
            input_events.write(InputActionMessage {
                entity,
                action: InputAction::Move(move_vec.normalize()),
            });
        }

        // A/D rotate only when left mouse is NOT held.
        if !strafe_mode {
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

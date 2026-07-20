use bevy::prelude::*;
use bevy::input::gamepad::{Gamepad, GamepadAxis};
use crate::runtime::messages::*;
use crate::runtime::scene_manager::LoadedKeyBindings;
use crate::capabilities::player::CharacterController;
use crate::schema::AppState;
use crate::schema::player::InputMap;

/// Resolves a player's `gamepad_index` against a slice of connected gamepads already sorted by
/// `Entity::index()` (built once per system per frame — see callers). `None` if the player has
/// no `gamepad_index` bound, or the index is out of range (fewer gamepads connected than
/// expected).
pub(crate) fn resolve_gamepad<'a>(
    sorted: &'a [(Entity, &'a Gamepad)],
    index: Option<usize>,
) -> Option<&'a Gamepad> {
    index.and_then(|i| sorted.get(i)).map(|(_, gp)| *gp)
}

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
    gamepad_query: Query<(Entity, &Gamepad)>,
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

    // Bevy has no built-in numeric gamepad index — each connected pad is its own entity.
    // Sort by entity index so `InputMap.gamepad_index: 0` consistently means "whichever
    // gamepad connected first this session" (good enough for local co-op; no rebinding UI).
    let mut sorted_gamepads: Vec<(Entity, &Gamepad)> = gamepad_query.iter().collect();
    sorted_gamepads.sort_by_key(|(e, _)| e.index());

    for (entity, controller) in &query {
        let gamepad = resolve_gamepad(&sorted_gamepads, controller.inputs.gamepad_index);
        let deadzone = controller.inputs.gamepad_deadzone;

        let strafe_mode = controller.inputs.strafe_mouse_button
            .as_deref()
            .and_then(InputMap::parse_mouse_button)
            .map(|btn| mouse_input.pressed(btn))
            .unwrap_or(false);
        let mut move_vec = Vec2::ZERO;
        let mut turn = 0.0;

        if let Some(key) = controller.inputs.key("forward") {
            if keyboard_input.pressed(key) { move_vec.y += 1.0; }
        }
        if let Some(key) = controller.inputs.key("backward") {
            if keyboard_input.pressed(key) { move_vec.y -= 1.0; }
        }
        // A/D strafe only when left mouse is held; otherwise A/D rotates instead.
        if strafe_mode {
            if let Some(key) = controller.inputs.key("right") {
                if keyboard_input.pressed(key) { move_vec.x += 1.0; }
            }
            if let Some(key) = controller.inputs.key("left") {
                if keyboard_input.pressed(key) { move_vec.x -= 1.0; }
            }
        } else {
            if let Some(key) = controller.inputs.key("left") {
                if keyboard_input.pressed(key) { turn += 1.0; }
            }
            if let Some(key) = controller.inputs.key("right") {
                if keyboard_input.pressed(key) { turn -= 1.0; }
            }
        }

        // Gamepad, when bound: left stick strafes/moves, right stick turns — independent of
        // the keyboard's strafe_mode (that toggle only exists to disambiguate A/D on a
        // keyboard; a gamepad already has separate sticks for move and turn).
        if let Some(gp) = gamepad {
            let lx = gp.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
            let ly = gp.get(GamepadAxis::LeftStickY).unwrap_or(0.0);
            if lx.abs() > deadzone { move_vec.x += lx; }
            if ly.abs() > deadzone { move_vec.y += ly; }

            let rx = gp.get(GamepadAxis::RightStickX).unwrap_or(0.0);
            if rx.abs() > deadzone { turn -= rx; }
        }

        // `clamp_length_max` (not `normalize`) so a fully-analog gamepad tilt keeps its
        // magnitude — only a diagonal keyboard press (length > 1) gets scaled down.
        if move_vec != Vec2::ZERO {
            input_events.write(InputActionMessage {
                entity,
                action: InputAction::Move(move_vec.clamp_length_max(1.0)),
            });
        }

        if turn != 0.0 {
            input_events.write(InputActionMessage {
                entity,
                action: InputAction::Turn(turn),
            });
        }

        let gamepad_jump = controller.inputs.gamepad_button("jump");
        let jump_pressed = controller.inputs.key("jump")
            .map(|k| keyboard_input.just_pressed(k))
            .unwrap_or(false)
            || gamepad.zip(gamepad_jump).map(|(gp, btn)| gp.just_pressed(btn)).unwrap_or(false);
        if jump_pressed {
            input_events.write(InputActionMessage {
                entity,
                action: InputAction::Jump(true),
            });
        }

        let gamepad_run = controller.inputs.gamepad_button("run");
        let run_pressed = controller.inputs.key("run")
            .map(|k| keyboard_input.just_pressed(k))
            .unwrap_or(false)
            || gamepad.zip(gamepad_run).map(|(gp, btn)| gp.just_pressed(btn)).unwrap_or(false);
        if run_pressed {
            input_events.write(InputActionMessage {
                entity,
                action: InputAction::Run(true),
            });
        }
    }
}

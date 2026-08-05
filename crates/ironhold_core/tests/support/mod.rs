#![allow(dead_code, unused_imports)]

use bevy::prelude::*;
use bevy::input::gamepad::{
    Gamepad, GamepadConnection, GamepadConnectionEvent, GamepadButton, GamepadAxis,
    RawGamepadEvent, RawGamepadButtonChangedEvent, RawGamepadAxisChangedEvent,
    gamepad_connection_system, gamepad_event_processing_system,
};
use bevy::input::ButtonState;
use ironhold_core::{GamePlugin, ProjectConfigPath, ProjectRoot};
use ironhold_core::runtime::{UiEvent, GameEvent, SceneEvent, InputActionMessage};

pub fn setup_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
       .add_plugins(bevy::state::app::StatesPlugin)
       .add_plugins(bevy::transform::TransformPlugin)
       .add_plugins(AssetPlugin::default())
       .add_plugins(bevy::scene::ScenePlugin)
       .add_message::<bevy::input::mouse::MouseMotion>()
       .add_message::<bevy::input::mouse::MouseWheel>()
       .init_resource::<ButtonInput<KeyCode>>()
       .init_resource::<ButtonInput<MouseButton>>()
       .init_resource::<Messages<UiEvent>>()
       .init_resource::<Messages<GameEvent>>()
       .init_resource::<Messages<SceneEvent>>()
       .init_resource::<Messages<InputActionMessage>>()
       .init_resource::<Messages<AppExit>>()
       .init_resource::<Messages<bevy::input::mouse::MouseMotion>>()
       .init_resource::<Messages<bevy::input::mouse::MouseWheel>>()
       .init_asset::<Mesh>()
       .init_asset::<bevy::shader::Shader>()
       .init_asset::<ironhold_core::capabilities::terrain_material::TerrainMaterial>()
       .init_asset::<StandardMaterial>()
       .init_asset::<Image>()
       .init_asset::<Scene>()
       .init_asset::<Gltf>()
       .init_asset::<AnimationGraph>()
       .init_asset::<ironhold_core::schema::player::AnimationPolicy>()
       .init_asset::<ironhold_core::schema::project::LogicRulesAsset>()
       .init_asset::<ironhold_core::schema::project::StateMachineAsset>()
       .init_asset::<bevy::audio::AudioSource>()
       .insert_resource(ProjectConfigPath("projects/integration_tests/integration_tests.project.ron".to_string()))
       .insert_resource(ProjectRoot("projects/integration_tests".to_string()))
       // Gamepad connection/event processing — turns simulated RawGamepadEvent/
       // GamepadConnectionEvent messages (see connect_test_gamepad/press_gamepad_button/
       // set_gamepad_axis below) into a real `Gamepad` component's digital/analog state, the
       // same systems Bevy's own GilrsPlugin would normally drive from real hardware. Harmless
       // to register unconditionally — tests that don't touch gamepads never send these events.
       .add_message::<bevy::input::gamepad::GamepadEvent>()
       .add_message::<GamepadConnectionEvent>()
       .add_message::<RawGamepadButtonChangedEvent>()
       .add_message::<bevy::input::gamepad::GamepadButtonChangedEvent>()
       .add_message::<bevy::input::gamepad::GamepadButtonStateChangedEvent>()
       .add_message::<bevy::input::gamepad::GamepadAxisChangedEvent>()
       .add_message::<RawGamepadAxisChangedEvent>()
       .add_message::<RawGamepadEvent>()
       .add_systems(
           PreUpdate,
           (gamepad_connection_system, gamepad_event_processing_system.after(gamepad_connection_system)),
       )
       .add_plugins(GamePlugin);
    app
}

/// Simulates a freshly-connected gamepad, returning its `Entity` (the value to use as a
/// player's `InputMap.gamepad_index` *seed* — index into connection order, sorted by
/// `Entity::index()`, same sort `gamepad_bind_system` uses to resolve a pending player's seed
/// into a `BoundGamepad`; see `gamepad_player_binding_hardening.md`. The old crate-shared
/// `resolve_gamepad` helper this comment used to reference was deleted along with that refactor
/// — every consumer now either reads `BoundGamepad` directly or re-sorts inline). Call
/// `app.update()` once after this (and before pressing any button/setting any axis) so
/// `gamepad_connection_system` has spawned the real `Gamepad` component before the input systems
/// under test read it.
pub fn connect_test_gamepad(app: &mut App) -> Entity {
    let gamepad = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<Messages<GamepadConnectionEvent>>()
        .write(GamepadConnectionEvent::new(
            gamepad,
            GamepadConnection::Connected {
                name: "Test Gamepad".to_string(),
                vendor_id: None,
                product_id: None,
            },
        ));
    gamepad
}

/// Simulates a button press on a connected gamepad (see `connect_test_gamepad`). Takes effect
/// on the next `app.update()` — `just_pressed` is true for exactly that one frame, mirroring
/// real hardware input.
pub fn press_gamepad_button(app: &mut App, gamepad: Entity, button: GamepadButton) {
    app.world_mut()
        .resource_mut::<Messages<RawGamepadEvent>>()
        .write(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
            gamepad, button, 1.0,
        )));
}

/// Simulates an analog stick/axis value on a connected gamepad (see `connect_test_gamepad`).
/// Takes effect on the next `app.update()`.
pub fn set_gamepad_axis(app: &mut App, gamepad: Entity, axis: GamepadAxis, value: f32) {
    app.world_mut()
        .resource_mut::<Messages<RawGamepadEvent>>()
        .write(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(gamepad, axis, value)));
}


use bevy::prelude::*;

// When the `inspector` feature is enabled, compile in the egui-based world inspector.
#[cfg(feature = "inspector")]
use bevy_inspector_egui::{
    bevy_egui::EguiPlugin,
    quick::WorldInspectorPlugin,
};

/// Stores whether the inspector UI is currently enabled.
/// This resource persists across scene loads.
#[cfg(feature = "inspector")]
#[derive(Resource, Default)]
pub struct InspectorEnabled(pub bool);

/// Toggles the inspector on key press (native: F12, web: Backquote).
#[cfg(feature = "inspector")]
fn toggle_inspector_key(
    keys: Res<ButtonInput<KeyCode>>,
    mut enabled: ResMut<InspectorEnabled>,
) {
    // Browsers reserve F12 for DevTools, so use a different key on wasm.
    #[cfg(target_arch = "wasm32")]
    const TOGGLE_KEY: KeyCode = KeyCode::Backquote;

    #[cfg(not(target_arch = "wasm32"))]
    const TOGGLE_KEY: KeyCode = KeyCode::F12;

    if keys.just_pressed(TOGGLE_KEY) {
        enabled.0 = !enabled.0;
        info!("InspectorEnabled = {}", enabled.0);
    }
}

/// Adds an in-game inspector UI for debugging.
/// - Native toggle: F12
/// - Web toggle: Backquote (`)
#[cfg(feature = "inspector")]
pub fn add_inspector_plugins(app: &mut App) {
    app.init_resource::<InspectorEnabled>()
        // Make sure this runs every frame to catch key presses
        .add_systems(Update, toggle_inspector_key)
        // Use default plugin config to avoid struct-field breakage across versions
        .add_plugins(EguiPlugin::default())
        // Only draw inspector when enabled
        .add_plugins(WorldInspectorPlugin::new().run_if(|enabled: Res<InspectorEnabled>| enabled.0));
}

/// No-op when the `inspector` feature is disabled.
#[cfg(not(feature = "inspector"))]
pub fn add_inspector_plugins(_app: &mut App) {}

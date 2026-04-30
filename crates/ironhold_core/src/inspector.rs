
use bevy::prelude::*;

// When the `inspector` feature is enabled, compile in the egui-based world inspector.
#[cfg(feature = "inspector")]
use bevy_inspector_egui::{
    bevy_egui::{EguiContexts, EguiPlugin, EguiContext},
    bevy_inspector,
};

/// Stores whether the inspector UI is currently enabled.
/// This resource persists across scene loads.
#[cfg(feature = "inspector")]
#[derive(Resource, Default)]
pub struct InspectorEnabled(pub bool);

/// Marker for the dedicated inspector camera.
#[cfg(feature = "inspector")]
#[derive(Component)]
pub struct InspectorCamera;

/// Spawns a dedicated camera for the inspector to ensure it stays on top.
#[cfg(feature = "inspector")]
fn setup_inspector_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("Inspector Camera"),
        Camera2d,
        InspectorCamera,
        bevy::prelude::Camera {
            order: 9999, // Extremely high to be above all other cameras
            clear_color: ClearColorConfig::None,
            ..default()
        },
        // Use a dedicated render layer to avoid rendering anything else (3D or UI)
        bevy::camera::visibility::RenderLayers::layer(31),
        // Add EguiContext so bevy_egui knows to render to this camera
        EguiContext::default(),
    ));
}

/// Toggles physics collider wireframes on F9.
/// Uses `Option<ResMut>` so it silently no-ops in headless tests where the
/// `RapierDebugRenderPlugin` (and its `DebugRenderContext` resource) is absent.
#[cfg(feature = "inspector")]
fn toggle_physics_debug_key(
    keys: Res<ButtonInput<KeyCode>>,
    debug_ctx: Option<ResMut<bevy_rapier3d::render::DebugRenderContext>>,
) {
    let Some(mut ctx) = debug_ctx else { return };
    if keys.just_pressed(KeyCode::F9) {
        ctx.enabled = !ctx.enabled;
        info!("PhysicsDebug = {}", ctx.enabled);
    }
}

/// Toggles the inspector on key press (native: F12, web: Backquote, or Escape to close).
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

    // Also allow Escape to close the inspector if it's already enabled.
    if enabled.0 && keys.just_pressed(KeyCode::Escape) {
        enabled.0 = false;
        info!("InspectorEnabled = false (ESC)");
    }
}

/// System to show the world inspector in a window that is always on top.
#[cfg(feature = "inspector")]
fn world_inspector_system(
    world: &mut World,
    mut system_state: Local<Option<bevy::ecs::system::SystemState<(Query<Entity, With<InspectorCamera>>, EguiContexts)>>>,
) {
    let enabled = world.resource::<InspectorEnabled>().0;
    if !enabled {
        return;
    }

    // Initialize SystemState if not already done.
    if system_state.is_none() {
        *system_state = Some(bevy::ecs::system::SystemState::new(world));
    }

    let (ctx, _camera_entity) = {
        let state = system_state.as_mut().unwrap();
        let (camera_query, mut contexts) = state.get_mut(world);
        
        let Some(camera_entity) = camera_query.iter().next() else {
            return;
        };

        let ctx = contexts.ctx_for_entity_mut(camera_entity).expect("Egui context not found").clone();
        state.apply(world);
        (ctx, camera_entity)
    };

    use bevy_inspector_egui::bevy_egui::egui;
    egui::Window::new("World Inspector")
        // Middle is draggable. Since this is on a dedicated camera with order 200, 
        // it will be on top of EVERYTHING regardless of the internal egui order.
        .order(egui::Order::Middle)
        .movable(true)
        .vscroll(true)
        .show(&ctx, |ui| {
            bevy_inspector::ui_for_world(world, ui);
        });
}

/// Adds an in-game inspector UI for debugging.
/// - Native toggle: F12
/// - Web toggle: Backquote (`)
#[cfg(feature = "inspector")]
pub fn add_inspector_plugins(app: &mut App) {
    app.init_resource::<InspectorEnabled>()
        .add_systems(Startup, setup_inspector_camera)
        .add_systems(Update, toggle_inspector_key)
        .add_systems(Update, toggle_physics_debug_key)
        .add_plugins(EguiPlugin::default())
        .add_plugins(bevy_inspector_egui::DefaultInspectorConfigPlugin)
        .add_systems(Update, world_inspector_system);
}

/// No-op when the `inspector` feature is disabled.
#[cfg(not(feature = "inspector"))]
pub fn add_inspector_plugins(_app: &mut App) {}

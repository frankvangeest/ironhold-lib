use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        // Always register CollisionEvent so MessageReader<CollisionEvent> is safe in all
        // contexts, including headless tests where RapierPhysicsPlugin is skipped.
        app.add_message::<CollisionEvent>();
        #[cfg(not(test))]
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());

        // Physics collider overlay — starts disabled; toggle with F9 when the
        // inspector feature is enabled. Gated by not(test) for the same reason
        // as RapierPhysicsPlugin: the resource won't exist in headless tests.
        #[cfg(all(not(test), feature = "inspector"))]
        app.add_plugins(bevy_rapier3d::render::RapierDebugRenderPlugin {
            enabled: false,
            ..default()
        });
    }
}

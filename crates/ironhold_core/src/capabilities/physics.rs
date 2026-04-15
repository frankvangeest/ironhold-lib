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
        
        // Configure gravity (default is -9.81 on Y, which is fine)
        // app.insert_resource(RapierConfiguration {
        //     gravity: Vec3::Y * -9.81,
        //     ..default()
        // });
    }
}

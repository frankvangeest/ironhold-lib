use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());
        
        // Configure gravity (default is -9.81 on Y, which is fine)
        // app.insert_resource(RapierConfiguration {
        //     gravity: Vec3::Y * -9.81,
        //     ..default()
        // });
    }
}

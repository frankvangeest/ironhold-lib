use bevy::prelude::*;

/// Continuously rotates an entity around its local Y-axis each frame.
/// Purely visual — runs in `Update`, not `FixedUpdate`.
#[derive(Component)]
pub struct Spin {
    /// Rotation speed in radians per second.
    pub speed: f32,
}

impl Default for Spin {
    fn default() -> Self {
        Self { speed: 2.5 }
    }
}

/// Rotates all `Spin` entities around their local Y axis each frame.
pub fn spin_system(
    time: Res<Time>,
    mut query: Query<(&Spin, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (spin, mut transform) in &mut query {
        transform.rotate_y(spin.speed * dt);
    }
}

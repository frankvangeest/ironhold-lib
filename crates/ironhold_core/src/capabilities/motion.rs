use bevy::prelude::*;

/// Continuously animates an entity's transform each frame.
/// Purely visual — runs in `Update`, not `FixedUpdate`.
///
/// All rotations are applied in **world space** so the effect is stable
/// regardless of the entity's initial orientation (e.g. a coin tilted 90°
/// around X will still spin correctly around the world Y axis).
///
/// Set `bob_origin_y` to the entity's spawn Y when inserting this component;
/// the bob oscillates around that value.
#[derive(Component, Default)]
pub struct Motion {
    /// World-space continuous rotation in radians per second (x, y, z axes).
    /// Each non-zero axis contributes an independent world-space rotation each frame.
    pub rotate: Vec3,
    /// Sinusoidal vertical bob: `(amplitude_m, frequency_hz)`.
    /// Requires `bob_origin_y` to be set; otherwise bob is skipped.
    pub bob: Option<(f32, f32)>,
    /// Y coordinate the bob oscillates around. Set from the entity's spawn translation.
    pub bob_origin_y: Option<f32>,
}

/// Applies `Motion` to all tagged entities each frame.
pub fn motion_system(
    time: Res<Time>,
    mut query: Query<(&Motion, &mut Transform)>,
) {
    let elapsed = time.elapsed_secs();
    let dt = time.delta_secs();

    for (motion, mut transform) in &mut query {
        // World-space rotation: pre-multiply so the axis stays world-aligned
        // even when the entity has an initial tilt (e.g. a standing coin).
        if motion.rotate != Vec3::ZERO {
            if motion.rotate.x != 0.0 {
                transform.rotation =
                    Quat::from_rotation_x(motion.rotate.x * dt) * transform.rotation;
            }
            if motion.rotate.y != 0.0 {
                transform.rotation =
                    Quat::from_rotation_y(motion.rotate.y * dt) * transform.rotation;
            }
            if motion.rotate.z != 0.0 {
                transform.rotation =
                    Quat::from_rotation_z(motion.rotate.z * dt) * transform.rotation;
            }
        }

        // Sinusoidal vertical bob.
        if let Some((amplitude, frequency)) = motion.bob {
            if let Some(origin_y) = motion.bob_origin_y {
                transform.translation.y =
                    origin_y + amplitude * (elapsed * frequency * std::f32::consts::TAU).sin();
            }
        }
    }
}

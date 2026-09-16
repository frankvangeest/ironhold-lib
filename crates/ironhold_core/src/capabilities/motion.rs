use bevy::prelude::*;
use crate::det_math;

/// Continuously animates an entity's transform each `FixedUpdate` tick (64Hz — not once per
/// rendered frame; see `planning/features/deterministic_fixed_timestep.md`). Moved here from
/// `Update`: a motion-carrying entity can also carry a collider/sensor, and writing a
/// wall-clock-driven transform into the physics world once per *rendered frame* was itself a
/// source of cross-platform/cross-framerate divergence, independent of the math used to compute
/// the rotation/bob. Accepted tradeoff: on a display refreshing faster than 64Hz, motion now
/// visibly steps rather than updating every rendered frame (no render-side interpolation — see
/// the feature plan's render-smoothing decision).
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
    /// Each non-zero axis contributes an independent world-space rotation each tick.
    pub rotate: Vec3,
    /// Sinusoidal vertical bob: `(amplitude_m, frequency_hz)`.
    /// Requires `bob_origin_y` to be set; otherwise bob is skipped.
    pub bob: Option<(f32, f32)>,
    /// Y coordinate the bob oscillates around. Set from the entity's spawn translation.
    pub bob_origin_y: Option<f32>,
}

/// Applies `Motion` to all tagged entities each `FixedUpdate` tick.
pub fn motion_system(
    time: Res<Time>,
    mut query: Query<(&Motion, &mut Transform)>,
) {
    let elapsed = time.elapsed_secs();
    let dt = time.delta_secs();

    for (motion, mut transform) in &mut query {
        // World-space rotation: pre-multiply so the axis stays world-aligned
        // even when the entity has an initial tilt (e.g. a standing coin).
        // `det_math::quat_from_rotation_*`, not `Quat::from_rotation_*` — routes the underlying
        // sin/cos through `libm` for cross-platform consistency (see `det_math`'s doc comment,
        // including its "Known residual" note on the quaternion multiply that follows).
        if motion.rotate != Vec3::ZERO {
            if motion.rotate.x != 0.0 {
                transform.rotation =
                    det_math::quat_from_rotation_x(motion.rotate.x * dt) * transform.rotation;
            }
            if motion.rotate.y != 0.0 {
                transform.rotation =
                    det_math::quat_from_rotation_y(motion.rotate.y * dt) * transform.rotation;
            }
            if motion.rotate.z != 0.0 {
                transform.rotation =
                    det_math::quat_from_rotation_z(motion.rotate.z * dt) * transform.rotation;
            }
        }

        // Sinusoidal vertical bob.
        if let Some((amplitude, frequency)) = motion.bob {
            if let Some(origin_y) = motion.bob_origin_y {
                transform.translation.y =
                    origin_y + amplitude * det_math::sin(elapsed * frequency * std::f32::consts::TAU);
            }
        }
    }
}

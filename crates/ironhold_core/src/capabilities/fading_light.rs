use bevy::prelude::*;

/// Maximum simultaneous fading lights. Bevy's clustered forward renderer handles a moderate
/// number of dynamic point lights on WebGPU, but mobile WebGPU tile/cluster limits can be
/// as low as 32 total (scene fixtures + dynamic). 16 dynamic lights leaves comfortable
/// headroom for authored scene point lights. Spawns beyond this cap are silently skipped
/// (particles still fire).
pub const MAX_FADING_LIGHTS: usize = 16;

/// Drives a temporary PointLight that fades in, holds, then fades out and despawns.
/// Spawned by `drain_particle_effects_system` when an `EffectDef` has a `light` block.
#[derive(Component)]
pub struct FadingLight {
    pub peak_intensity: f32,
    pub fade_in_secs: f32,
    pub fade_out_secs: f32,
    pub duration_secs: f32,
    pub elapsed: f32,
}

pub fn fading_light_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut FadingLight, &mut PointLight)>,
) {
    let dt = time.delta_secs();
    for (entity, mut fading, mut light) in &mut query {
        fading.elapsed += dt;
        let t = fading.elapsed / fading.duration_secs;
        let fade_in_t = if fading.fade_in_secs > 0.0 {
            (fading.elapsed / fading.fade_in_secs).min(1.0)
        } else {
            1.0
        };
        let hold_end = fading.duration_secs - fading.fade_out_secs;
        let fade_out_t = if fading.fade_out_secs > 0.0 && fading.elapsed > hold_end {
            1.0 - ((fading.elapsed - hold_end) / fading.fade_out_secs).min(1.0)
        } else {
            1.0
        };
        light.intensity = fading.peak_intensity * fade_in_t * fade_out_t;
        if t >= 1.0 {
            commands.entity(entity).despawn();
        }
    }
}

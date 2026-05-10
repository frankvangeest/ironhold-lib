use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use crate::schema::stats::LoadedStats;

// ---------------------------------------------------------------------------
// Uniform layout
// ---------------------------------------------------------------------------

/// 7 × vec4 = 112 bytes; all fields 16-byte aligned for WebGPU.
#[derive(ShaderType, Clone, Default, PartialEq)]
pub struct RadarUniforms {
    /// Stat ratios 0-3 (0.0 = empty, 1.0 = full).
    pub ratios_0: Vec4,
    /// Stat ratios 4-7.
    pub ratios_1: Vec4,
    /// x = stat_count, y = grid_steps, z = outline_width (UV fraction), w = unused.
    pub config: Vec4,
    pub fill_color: Vec4,
    pub outline_color: Vec4,
    pub grid_color: Vec4,
    pub background_color: Vec4,
}

// ---------------------------------------------------------------------------
// Material asset
// ---------------------------------------------------------------------------

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct RadarMaterial {
    #[uniform(0)]
    pub uniforms: RadarUniforms,
}

impl Default for RadarMaterial {
    fn default() -> Self {
        Self {
            uniforms: RadarUniforms {
                ratios_0: Vec4::ZERO,
                ratios_1: Vec4::ZERO,
                config: Vec4::new(5.0, 3.0, 0.008, 0.0),
                fill_color: Vec4::new(0.35, 0.65, 1.0, 0.45),
                outline_color: Vec4::new(0.55, 0.85, 1.0, 1.0),
                grid_color: Vec4::new(0.40, 0.45, 0.55, 0.45),
                background_color: Vec4::new(0.10, 0.12, 0.20, 0.80),
            },
        }
    }
}

impl UiMaterial for RadarMaterial {
    fn fragment_shader() -> ShaderRef {
        "shared/shaders/custom_stat_radar.wgsl".into()
    }
}

// ---------------------------------------------------------------------------
// Runtime component + update system
// ---------------------------------------------------------------------------

/// Placed on the `StatRadar` UI entity by the scene loader.
/// `stat_radar_update_system` reads `LoadedStats` and writes ratios to the
/// `RadarMaterial` uniform each frame, guarding writes to avoid spurious
/// change-detection.
#[derive(Component, Clone)]
pub struct StatRadarNode {
    pub stat_keys: Vec<String>,
}

/// Updates every `RadarMaterial` instance by reading current stat ratios
/// from `LoadedStats`. Guarded write: only marks the asset changed when
/// the ratios actually differ.
pub fn stat_radar_update_system(
    loaded_stats: Res<LoadedStats>,
    query: Query<(&StatRadarNode, &MaterialNode<RadarMaterial>)>,
    mut materials: ResMut<Assets<RadarMaterial>>,
) {
    for (node, mat_node) in query.iter() {
        let Some(mat) = materials.get_mut(&mat_node.0) else { continue };

        let mut r = [0.0f32; 8];
        for (i, key) in node.stat_keys.iter().enumerate().take(8) {
            if let Some(stat) = loaded_stats.0.get(key) {
                let range = stat.def.max - stat.def.min;
                r[i] = if range <= 0.0 {
                    1.0
                } else {
                    ((stat.current - stat.def.min) / range).clamp(0.0, 1.0)
                };
            }
        }

        let new_0 = Vec4::new(r[0], r[1], r[2], r[3]);
        let new_1 = Vec4::new(r[4], r[5], r[6], r[7]);
        if mat.uniforms.ratios_0 != new_0 || mat.uniforms.ratios_1 != new_1 {
            mat.uniforms.ratios_0 = new_0;
            mat.uniforms.ratios_1 = new_1;
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct StatRadarPlugin;

impl Plugin for StatRadarPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<RadarMaterial>::default());
    }
}

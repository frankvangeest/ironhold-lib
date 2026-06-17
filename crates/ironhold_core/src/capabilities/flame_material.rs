use bevy::prelude::*;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError,
};
use bevy::shader::{Shader, ShaderRef};
use bevy::pbr::Material;
use bevy::asset::uuid_handle;

pub const FLAME_PARTICLE_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("666c616d-6570-4172-8172-666c616d6101");

// ─── Uniforms ────────────────────────────────────────────────────────────────

/// GPU-side uniforms for `FlameParticleMaterial` — 2 × `vec4<f32>` = 32 bytes.
/// 16-byte aligned per WebGPU requirements.
#[derive(ShaderType, Clone, Default)]
pub struct FlameUniforms {
    /// Linear-space RGBA tint.  Updated every frame from the particle colour gradient.
    pub color: Vec4,
    /// (scroll_speed, distort_strength, _unused_, elapsed_time).
    /// `elapsed_time` is updated every frame from `Particle::elapsed`.
    pub params: Vec4,
}

// ─── Material ────────────────────────────────────────────────────────────────

/// Purpose-built `Material` for animated flame/fire billboard particles.
///
/// Differences from `StandardMaterial`:
/// - Always unlit, always additive, always double-sided.
/// - Exposes `elapsed_time` and `distort_strength` uniforms so the WGSL shader
///   can animate UV distortion and scroll without any external clock resource.
/// - One texture slot for the flame sprite (Kenney Particle Pack or similar).
///
/// `particle_system` updates `uniforms.color` and `uniforms.params.w` (elapsed
/// time) every frame; `drain_particle_effects_system` creates one instance per
/// particle when `EffectDef.uv_distort > 0` or `EffectDef.uv_scroll_speed > 0`.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct FlameParticleMaterial {
    #[uniform(0)]
    pub uniforms: FlameUniforms,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
}

impl Material for FlameParticleMaterial {
    fn fragment_shader() -> ShaderRef {
        FLAME_PARTICLE_SHADER_HANDLE.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &bevy_mesh::MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

// ─── Plugin ──────────────────────────────────────────────────────────────────

pub struct FlameParticleMaterialPlugin;

impl Plugin for FlameParticleMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<FlameParticleMaterial>::default());
        app.add_systems(Startup, setup_flame_particle_shader);
    }
}

fn setup_flame_particle_shader(mut shaders: ResMut<Assets<Shader>>) {
    let _ = shaders.insert(
        &FLAME_PARTICLE_SHADER_HANDLE,
        Shader::from_wgsl(
            include_str!("../../../../assets/shared/shaders/custom_flame_particle.wgsl"),
            "shared/shaders/custom_flame_particle.wgsl",
        ),
    );
}

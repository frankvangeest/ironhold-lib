use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TerrainMaterial {
    #[uniform(0)]
    pub uv_scale: f32,

    #[texture(1)]
    #[sampler(2)]
    pub splatmap: Handle<Image>,

    #[texture(3)]
    #[sampler(4)]
    pub texture_r: Handle<Image>,

    #[texture(5)]
    #[sampler(6)]
    pub texture_g: Handle<Image>,

    #[texture(7)]
    #[sampler(8)]
    pub texture_b: Handle<Image>,

    #[texture(9)]
    #[sampler(10)]
    pub texture_a: Handle<Image>,
}

use bevy::shader::ShaderRef;

impl Material for TerrainMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain.wgsl".into()
    }
}

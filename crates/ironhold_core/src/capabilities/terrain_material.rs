use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::{Shader, ShaderRef};

use bevy::asset::Handle;
use bevy::asset::uuid_handle;

pub const TERRAIN_SHADER_HANDLE: Handle<Shader> = uuid_handle!("74657272-6169-4e5f-8d61-746572696101");

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TerrainMaterial {
    #[uniform(0)]
    pub uv_scale: Vec4,  // Only .x is used; padded to 16 bytes for WebGPU alignment

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

impl Material for TerrainMaterial {
    fn fragment_shader() -> ShaderRef {
        TERRAIN_SHADER_HANDLE.into()
    }
}

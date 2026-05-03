use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialDef {
    pub kind: MaterialKind,

    #[serde(default)]
    pub alpha_mode: AlphaModeDef,

    #[serde(default)]
    pub double_sided: bool,

    #[serde(default)]
    pub unlit: bool,

    #[serde(default)]
    pub uv_transform: Option<UvTransformDef>,

    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MaterialKind {
    Standard(StandardMaterialDef),
    Terrain(TerrainMaterialDef),
    Custom(CustomMaterialDef),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum AlphaModeDef {
    #[default]
    Opaque,
    Mask(f32),
    Blend,
    Premultiplied,
    Add,
    Multiply,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UvTransformDef {
    pub offset: Vec2Def,
    pub scale: Vec2Def,
    pub rotation_radians: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardMaterialDef {
    #[serde(default)]
    pub base_color: ColorDef,

    #[serde(default)]
    pub base_color_texture: Option<String>,

    #[serde(default)]
    pub normal_map_texture: Option<String>,

    #[serde(default)]
    pub metallic_roughness_texture: Option<String>,

    #[serde(default)]
    pub occlusion_texture: Option<String>,

    #[serde(default)]
    pub emissive_texture: Option<String>,

    #[serde(default = "default_emissive")]
    pub emissive: ColorDef,

    #[serde(default)]
    pub metallic: f32,

    #[serde(default = "default_perceptual_roughness")]
    pub perceptual_roughness: f32,

    #[serde(default)]
    pub reflectance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainMaterialDef {
    pub splatmap: String,
    pub layers: Vec<String>,
    /// UV tiling scale for terrain layer textures. Higher values tile textures more finely.
    /// Defaults to 10.0.
    #[serde(default = "default_terrain_uv_scale")]
    pub uv_scale: f32,
}

fn default_terrain_uv_scale() -> f32 { 10.0 }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomMaterialDef {
    #[serde(default)]
    pub shader: Option<String>,

    #[serde(default)]
    pub textures: HashMap<String, String>,

    #[serde(default)]
    pub floats: HashMap<String, f32>,

    #[serde(default)]
    pub colors: HashMap<String, ColorDef>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Vec2Def {
    pub x: f32,
    pub y: f32,
}

impl Default for Vec2Def {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ColorDef {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for ColorDef {
    fn default() -> Self {
        Self::WHITE
    }
}

impl ColorDef {
    pub const WHITE: Self = Self { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const BLACK: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
}

fn default_emissive() -> ColorDef {
    ColorDef::BLACK
}

fn default_perceptual_roughness() -> f32 {
    0.5
}
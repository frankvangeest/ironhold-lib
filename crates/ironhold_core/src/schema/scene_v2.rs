use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

/// Scene file format version 2.
#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct GameSceneV2 {
    pub schema_version: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub lighting: Option<SceneLightingV2>,
    #[serde(default)]
    pub terrain: Option<TerrainConfigV2>,
    #[serde(default)]
    pub spawn_points: HashMap<String, (f32, f32, f32)>,
    #[serde(default)]
    pub entities: Vec<SceneEntityDef>,
    #[serde(default)]
    pub ui: Vec<UiButtonDefV2>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SceneLightingV2 {
    #[serde(default)]
    pub ambient: Option<(f32, f32, f32)>,
    #[serde(default)]
    pub directional: Option<DirectionalLightDefV2>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DirectionalLightDefV2 {
    pub color: (f32, f32, f32),
    pub intensity: f32,
    pub rotation_euler_deg: (f32, f32, f32),
    #[serde(default = "default_true")]
    pub shadows_enabled: bool,
}

fn default_true() -> bool { true }

#[derive(Deserialize, Debug, Clone)]
pub struct TerrainConfigV2 {
    pub heightmap: String,
    pub splatmap: String,
    /// (horizontal_x, height_multiplier, horizontal_z) scale factors.
    pub scale: (f32, f32, f32),
    pub material_paths: Vec<String>,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u32,
}

fn default_chunk_size() -> u32 { 64 }

#[derive(Deserialize, Debug, Clone)]
pub struct SceneEntityDef {
    pub id: String,
    pub prefab: String,
    pub transform: SceneTransformV2,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct SceneTransformV2 {
    #[serde(default)]
    pub translation: (f32, f32, f32),
    #[serde(default)]
    pub rotation_euler_deg: (f32, f32, f32),
    #[serde(default = "one_vec3")]
    pub scale: (f32, f32, f32),
}

fn one_vec3() -> (f32, f32, f32) { (1.0, 1.0, 1.0) }

#[derive(Deserialize, Debug, Clone)]
pub struct UiButtonDefV2 {
    pub kind: String,
    pub id: String,
    pub text: String,
    /// Action trigger. "ui." prefix is stripped when firing (e.g. "ui.quit" → "quit").
    pub action: String,
    pub position: (f32, f32),
    pub size: (f32, f32),
}

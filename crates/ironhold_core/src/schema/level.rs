use bevy::prelude::*;
use serde::Deserialize;
use crate::schema::player::PlayerConfig;
use crate::schema::ui::UiElement;

#[derive(Deserialize, Debug, Clone, Component)]
#[serde(deny_unknown_fields)]
pub struct TerrainConfig {
    pub heightmap_path: String,
    pub splatmap_path: String,
    #[serde(default = "default_height_scale")]
    pub height_scale: f32,
    #[serde(default = "default_horizontal_scale")]
    pub horizontal_scale: f32,
    #[serde(default = "default_position")]
    pub position: (f32, f32, f32),
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u32,
    pub material_paths: Vec<String>,
}

fn default_height_scale() -> f32 { 100.0 }
fn default_horizontal_scale() -> f32 { 1.0 }
fn default_position() -> (f32, f32, f32) { (0.0, 0.0, 0.0) }
fn default_chunk_size() -> u32 { 64 }

pub const LEVEL_SCHEMA_VERSION: u32 = 1;

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct GameLevel {
    pub schema_version: u32,

    #[serde(default)]
    pub models: Vec<ModelInfo>,
    #[serde(default)]
    pub ui: Vec<UiElement>,
    #[serde(default)]
    pub player: Option<PlayerConfig>,
    #[serde(default)]
    pub terrain: Option<TerrainConfig>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelInfo {
    pub path: String,
    pub position: (f32, f32, f32),
}

#[derive(Resource)]
pub struct LevelHandle(pub Handle<GameLevel>);

#[derive(Component)]
pub struct LevelEntity;


impl GameLevel {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != LEVEL_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported GameLevel schema_version {} (expected {})",
                self.schema_version, LEVEL_SCHEMA_VERSION
            ));
        }
        Ok(())
    }
}


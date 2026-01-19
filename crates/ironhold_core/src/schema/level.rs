use bevy::prelude::*;
use serde::Deserialize;
use crate::schema::player::PlayerConfig;
use crate::schema::ui::UiElement;

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


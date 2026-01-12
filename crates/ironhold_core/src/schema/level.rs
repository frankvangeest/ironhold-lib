use bevy::prelude::*;
use serde::Deserialize;
use crate::schema::player::PlayerConfig;
use crate::schema::ui::UiElement;

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct GameLevel {
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

use bevy::prelude::*;
use serde::Deserialize;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum AppState {
    #[default]
    Bootstrap,
    LoadingProject,
    LoadingScene,
    InGame,
}

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct ProjectConfig {
    pub initial_scene: String,
}

#[derive(Resource)]
pub struct ProjectConfigHandle(pub Handle<ProjectConfig>);

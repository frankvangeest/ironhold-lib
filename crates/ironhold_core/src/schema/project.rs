use bevy::prelude::*;
use serde::Deserialize;

pub const PROJECT_SCHEMA_VERSION: u32 = 1;

use crate::schema::actions::Action;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum AppState {
    #[default]
    Bootstrap,
    LoadingProject,
    LoadingScene,
    InGame,
}

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub schema_version: u32,
    pub initial_scene: String,
    pub rules: Vec<LogicRule>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LogicRule {
    pub on: String,
    pub do_actions: Vec<Action>,
}

#[derive(Resource)]
pub struct ProjectConfigHandle(pub Handle<ProjectConfig>);


impl ProjectConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != PROJECT_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported ProjectConfig schema_version {} (expected {})",
                self.schema_version, PROJECT_SCHEMA_VERSION
            ));
        }
        Ok(())
    }
}

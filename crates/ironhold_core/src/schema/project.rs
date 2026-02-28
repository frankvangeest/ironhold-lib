use bevy::prelude::*;
use std::collections::HashMap;
use serde::{ 
    Serialize,
    Deserialize,
};

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

#[derive(Deserialize, Asset, TypePath, Debug, Clone, Resource)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub schema_version: u32,
    pub initial_scene: String,
    pub rules: Vec<LogicRule>,
    #[serde(default)]
    pub model_fixes: HashMap<String, TransformFix>,
    #[serde(default)]
    pub global_environment: Option<crate::schema::level::EnvironmentMapConfig>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformFix {
    #[serde(default)]
    pub pivot_offset: (f32, f32, f32),
    #[serde(default)]
    pub rotation_deg: (f32, f32, f32),
    #[serde(default = "one_vec3")]
    pub scale: (f32, f32, f32),
}

fn one_vec3() -> (f32, f32, f32) { (1.0, 1.0, 1.0) }

impl Default for TransformFix {
    fn default() -> Self {
        Self { pivot_offset: (0.0, 0.0, 0.0), rotation_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0) }
    }
}

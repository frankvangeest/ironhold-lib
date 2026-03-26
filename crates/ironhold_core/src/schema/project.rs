use bevy::prelude::*;
use std::collections::HashMap;
use serde::{
    Serialize,
    Deserialize,
};

use crate::schema::actions::Action;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum AppState {
    #[default]
    Bootstrap,
    LoadingProject,
    LoadingScene,
    InGame,
}

/// A standalone `.ron` asset that holds per-model transform corrections.
#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct ModelFixesAsset {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub model_fixes: HashMap<String, TransformFix>,
}

/// A standalone `.ron` asset that holds the logic rules for a project (schema v2).
#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct LogicRulesAsset {
    #[serde(default)]
    pub schema_version: u32,
    pub rules: Vec<LogicRule>,
}

#[derive(Deserialize, Asset, TypePath, Debug, Clone, Resource)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub schema_version: u32,
    pub initial_scene: String,

    // V1: inline logic rules
    #[serde(default)]
    pub rules: Vec<LogicRule>,
    // V2: path to external logic/rules.ron
    #[serde(default)]
    pub rules_path: Option<String>,

    // V1: inline per-model transform corrections
    #[serde(default)]
    pub model_fixes: HashMap<String, TransformFix>,
    // V1/V2: path to external overrides/model_fixes.ron
    #[serde(default)]
    pub model_fixes_path: Option<String>,

    // V2 metadata (stored, not yet used by the runtime)
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub asset_catalog: Option<String>,
    #[serde(default)]
    pub prefab_catalog: Option<String>,

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
        if self.schema_version != 1 && self.schema_version != 2 {
            return Err(format!(
                "Unsupported ProjectConfig schema_version {} (expected 1 or 2)",
                self.schema_version
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

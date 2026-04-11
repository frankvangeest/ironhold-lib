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
    pub schema_version: u32,
    #[serde(default)]
    pub model_fixes: HashMap<String, TransformFix>,
}

impl ModelFixesAsset {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version < 1 || self.schema_version > 2 {
            return Err(format!(
                "Unsupported ModelFixesAsset schema_version {} (expected 1 or 2)",
                self.schema_version
            ));
        }
        Ok(())
    }
}

/// A standalone `.ron` asset that holds the logic rules for a project (schema v2).
#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct LogicRulesAsset {
    pub schema_version: u32,
    pub rules: Vec<LogicRule>,
}

impl LogicRulesAsset {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version < 1 || self.schema_version > 2 {
            return Err(format!(
                "Unsupported LogicRulesAsset schema_version {} (expected 1 or 2)",
                self.schema_version
            ));
        }
        for (i, rule) in self.rules.iter().enumerate() {
            if rule.on.is_empty() {
                return Err(format!("LogicRule[{}] has empty \"on\" field", i));
            }
        }
        Ok(())
    }
}

/// A standalone `.ron` asset holding a finite-state machine definition (schema v1).
/// Replaces `logic/rules.ron` for projects that use the FSM authoring workflow.
/// Referenced via `state_machine_path` in the project config.
#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct StateMachineAsset {
    pub schema_version: u32,
    /// The logic state the machine starts in before any transitions fire.
    pub initial_state: String,
    /// Named states with entry/exit actions and in-state event bindings.
    pub states: Vec<FsmState>,
    /// State-change transitions triggered by events.
    pub transitions: Vec<FsmTransition>,
    /// Event bindings that fire from any state without changing state.
    #[serde(default)]
    pub global_on: Vec<FsmEventBinding>,
}

impl StateMachineAsset {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "Unsupported StateMachineAsset schema_version {} (expected 1)",
                self.schema_version
            ));
        }
        if self.initial_state.is_empty() {
            return Err("StateMachineAsset initial_state must not be empty".to_string());
        }
        let mut state_names = std::collections::HashSet::new();
        for state in &self.states {
            if state.name.is_empty() {
                return Err("FSM state has empty name".to_string());
            }
            if !state_names.insert(state.name.as_str()) {
                return Err(format!("Duplicate FSM state name: \"{}\"", state.name));
            }
        }
        if !self.states.is_empty() && !state_names.contains(self.initial_state.as_str()) {
            return Err(format!(
                "StateMachineAsset initial_state \"{}\" not found in states list",
                self.initial_state
            ));
        }
        for transition in &self.transitions {
            if !state_names.contains(transition.to.as_str()) {
                return Err(format!(
                    "FSM transition to \"{}\" references unknown state",
                    transition.to
                ));
            }
            if let Some(ref from) = transition.from {
                if !state_names.contains(from.as_str()) {
                    return Err(format!(
                        "FSM transition from \"{}\" references unknown state",
                        from
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct FsmState {
    pub name: String,
    /// Actions queued automatically when entering this state.
    #[serde(default)]
    pub entry_actions: Vec<Action>,
    /// Actions queued automatically when leaving this state.
    #[serde(default)]
    pub exit_actions: Vec<Action>,
    /// In-state event bindings; fire while in this state without changing state.
    #[serde(default)]
    pub on: Vec<FsmEventBinding>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct FsmTransition {
    /// Source state name. Omit (or `None`) to match any current state.
    #[serde(default)]
    pub from: Option<String>,
    /// Event string that triggers this transition (e.g. `"ui.button_pressed:start_game"`).
    pub on: String,
    /// Target state after the transition fires.
    pub to: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct FsmEventBinding {
    pub event: String,
    pub do_actions: Vec<Action>,
}

#[derive(Deserialize, Asset, TypePath, Debug, Clone, Resource, Default)]
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
    // V2: path to external logic/state_machine.ron (FSM workflow; replaces rules_path)
    #[serde(default)]
    pub state_machine_path: Option<String>,

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
    pub global_environment: Option<EnvironmentMapConfig>,

    /// Global key → event trigger mappings, applied regardless of which scene is active.
    /// Key names use the same string format as InputMap (e.g. "Escape", "Tab", "F1").
    /// The value is the trigger name fired as `ui.button_pressed:<trigger>`.
    /// Example: `{ "Escape": "toggle_pause" }` fires `ui.button_pressed:toggle_pause` on Escape.
    #[serde(default)]
    pub global_key_bindings: HashMap<String, String>,

    /// Default base color applied to every `kind: "primitive"` prefab that does not
    /// specify its own `primitive.color`. Expressed as linear sRGB (r, g, b) in the
    /// 0.0–1.0 range. When absent, the engine falls back to a neutral grey (0.7, 0.7, 0.7).
    #[serde(default)]
    pub primitive_default_color: Option<(f32, f32, f32)>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LogicRule {
    pub on: String,
    /// Optional logic-state guard. When set, the rule only fires while the interpreter
    /// is in the named state. When omitted (or `None`), the rule fires in every state.
    #[serde(default)]
    pub when: Option<String>,
    pub do_actions: Vec<Action>,
}

#[derive(Resource)]
pub struct ProjectConfigHandle(pub Handle<ProjectConfig>);


impl ProjectConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version < 1 || self.schema_version > 3 {
            return Err(format!(
                "Unsupported ProjectConfig schema_version {} (expected 1, 2, or 3)",
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

// ─── Terrain ──────────────────────────────────────────────────────────────────

/// Terrain mesh parameters. Placed as a `Component` on a spawned entity; the
/// `TerrainPlugin` systems detect it and kick off async mesh generation.
#[derive(Deserialize, Debug, Clone, Component)]
#[serde(deny_unknown_fields)]
pub struct TerrainConfig {
    pub heightmap_path: String,
    pub splatmap_path: String,
    #[serde(default = "default_height_scale")]
    pub height_scale: f32,
    #[serde(default = "default_horizontal_scale")]
    pub horizontal_scale: f32,
    #[serde(default = "default_terrain_position")]
    pub position: (f32, f32, f32),
    #[serde(default = "default_terrain_chunk_size")]
    pub chunk_size: u32,
    pub material_paths: Vec<String>,
}

fn default_height_scale() -> f32 { 100.0 }
fn default_horizontal_scale() -> f32 { 1.0 }
fn default_terrain_position() -> (f32, f32, f32) { (0.0, 0.0, 0.0) }
fn default_terrain_chunk_size() -> u32 { 64 }

// ─── Environment map ──────────────────────────────────────────────────────────

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentMapConfig {
    #[serde(default)]
    pub diffuse_path: Option<String>,
    #[serde(default)]
    pub specular_path: Option<String>,
    pub intensity: f32,
    #[serde(default)]
    pub fallback: Option<GeneratedEnvironmentMapLight>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct GeneratedEnvironmentMapLight {
    pub top_color: (f32, f32, f32),
    pub bottom_color: (f32, f32, f32),
}

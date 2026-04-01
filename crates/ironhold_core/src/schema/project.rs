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
    pub global_environment: Option<crate::schema::level::EnvironmentMapConfig>,

    /// Global key → event trigger mappings, applied regardless of which scene is active.
    /// Key names use the same string format as InputMap (e.g. "Escape", "Tab", "F1").
    /// The value is the trigger name fired as `ui.button_pressed:<trigger>`.
    /// Example: `{ "Escape": "toggle_pause" }` fires `ui.button_pressed:toggle_pause` on Escape.
    #[serde(default)]
    pub global_key_bindings: HashMap<String, String>,
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

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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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

    /// Global gamepad button → event trigger mappings, applied regardless of which scene is
    /// active. Button names use the same format as `InputMap`'s `gamepad_*` fields (e.g.
    /// "South", "Start", "DPadUp" — see `InputMap::parse_gamepad_button`). The value is the
    /// trigger name fired as `ui.button_pressed:<trigger>`. **Named `unclaimed` deliberately,
    /// unlike `global_key_bindings` — this is NOT a general gamepad analogue of that field.** A
    /// match only ever fires on a gamepad not currently assigned to any live player (including a
    /// player whose seed is about to resolve to it, even before their binding is written this
    /// exact frame) — an already-joined player's own button presses never reach this map, no
    /// matter what trigger name is bound here. Intended for join-style triggers (a new player
    /// pressing a button to enter the game) — see the "Local co-op hot join" docs. For an
    /// already-joined player's own in-game gamepad actions, use that player's own `InputMap`
    /// fields (`gamepad_jump`/`gamepad_run`/`gamepad_interact`/`gamepad_target_next`) instead.
    #[serde(default)]
    pub global_unclaimed_gamepad_bindings: HashMap<String, String>,

    /// Default base color applied to every `kind: "primitive"` prefab that does not
    /// specify its own `primitive.color`. Expressed as linear sRGB (r, g, b) in the
    /// 0.0–1.0 range. When absent, the engine falls back to a neutral grey (0.7, 0.7, 0.7).
    #[serde(default)]
    pub primitive_default_color: Option<(f32, f32, f32)>,

    /// Path to a `stats.ron` file that defines named stats (health, mana, etc.).
    /// Optional: when absent, the stat system is inactive for this project.
    /// Example: `"stats/stats.ron"`.
    #[serde(default)]
    pub stats_path: Option<String>,

    /// Path to an `items/items.ron` file that defines the item catalog for this project.
    /// Optional: when absent, the inventory system has no items.
    /// Example: `"items/items.ron"`.
    #[serde(default)]
    pub items_path: Option<String>,

    /// Visual style for floating damage/heal popups shown by `Action::ShowDamagePopup`.
    /// Omit to use the built-in defaults (22 px font, 1.2 s duration, 1.5 m/s rise).
    #[serde(default)]
    pub damage_popup_style: Option<DamagePopupStyle>,

    /// Project-level audio settings. Omit to use defaults (`max_volume: 1.0, mute_on_start: false`).
    #[serde(default)]
    pub audio: AudioConfig,
}

/// Project-level audio configuration. All fields have data-driven defaults so existing projects
/// that omit the `audio:` block behave identically to `max_volume: 1.0, mute_on_start: false`.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct AudioConfig {
    /// Master volume ceiling for this project (0.0–1.0). Effective `GlobalVolume` is
    /// `active_fraction * max_volume` when unmuted. Default: 1.0.
    #[serde(default = "default_max_volume")]
    pub max_volume: f32,
    /// Start the project muted. Default: false.
    #[serde(default)]
    pub mute_on_start: bool,
}

impl Default for AudioConfig {
    fn default() -> Self { Self { max_volume: default_max_volume(), mute_on_start: false } }
}

fn default_max_volume() -> f32 { 1.0 }

/// Visual style for `Action::ShowDamagePopup` popups. Set once per project in `.project.ron`.
/// All fields are optional — omit any to use the built-in default shown in the comment.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DamagePopupStyle {
    /// Font size in screen pixels. Default: 22.0.
    #[serde(default = "default_popup_font_size")]
    pub font_size: f32,
    /// Seconds the popup is visible before fading out completely. Default: 1.2.
    #[serde(default = "default_popup_duration")]
    pub duration_secs: f32,
    /// Metres per second the popup rises. Default: 1.5.
    #[serde(default = "default_popup_rise_speed")]
    pub rise_speed: f32,
    /// World-space offset from the entity origin where the popup spawns. Default: `(0.0, 1.2, 0.0)`.
    /// Increase Y for tall entities (bosses) so the label appears above the head.
    #[serde(default = "default_popup_spawn_offset")]
    pub spawn_offset: (f32, f32, f32),
    /// Colour for negative amounts (damage). Linear RGBA. Default: red `(0.95, 0.25, 0.20, 1.0)`.
    #[serde(default = "default_popup_damage_color")]
    pub damage_color: (f32, f32, f32, f32),
    /// Colour for positive amounts (healing). Linear RGBA. Default: green `(0.20, 0.90, 0.20, 1.0)`.
    #[serde(default = "default_popup_heal_color")]
    pub heal_color: (f32, f32, f32, f32),
}

impl Default for DamagePopupStyle {
    fn default() -> Self {
        Self {
            font_size: default_popup_font_size(),
            duration_secs: default_popup_duration(),
            rise_speed: default_popup_rise_speed(),
            spawn_offset: default_popup_spawn_offset(),
            damage_color: default_popup_damage_color(),
            heal_color: default_popup_heal_color(),
        }
    }
}

fn default_popup_font_size() -> f32 { 22.0 }
fn default_popup_duration() -> f32 { 1.2 }
fn default_popup_rise_speed() -> f32 { 1.5 }
fn default_popup_spawn_offset() -> (f32, f32, f32) { (0.0, 1.2, 0.0) }
fn default_popup_damage_color() -> (f32, f32, f32, f32) { (0.95, 0.25, 0.20, 1.0) }
fn default_popup_heal_color() -> (f32, f32, f32, f32) { (0.20, 0.90, 0.20, 1.0) }

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
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

// ─── Environment map ──────────────────────────────────────────────────────────

fn default_env_map_intensity() -> f32 { 1.0 }

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentMapConfig {
    #[serde(default)]
    pub diffuse_path: Option<String>,
    #[serde(default)]
    pub specular_path: Option<String>,
    #[serde(default = "default_env_map_intensity")]
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

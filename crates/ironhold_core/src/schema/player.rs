use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug, Clone)]
pub struct PlayerConfig {
    pub model_path: String,
    pub initial_position: (f32, f32, f32),
    pub camera: CameraConfig,
    pub inputs: InputMap,

    /// Data-driven animation policy.
    ///
    /// New abilities and animations can be added via RON without recompiling.
    pub animation_policy: AnimationPolicy,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CameraConfig {
    pub offset: (f32, f32, f32),
    pub look_at_offset: (f32, f32, f32),
    pub zoom_speed: f32,
    pub orbit_speed: f32,
    pub min_radius: f32,
    pub max_radius: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct InputMap {
    pub forward: String,
    pub backward: String,
    pub left: String,
    pub right: String,
    pub strafe_left: String,
    pub strafe_right: String,
    pub jump: String,
    #[serde(default = "default_run_key")]
    pub run: String,
}

fn default_run_key() -> String {
    "ShiftLeft".to_string()
}

impl InputMap {
    pub fn key(&self, name: &str) -> Option<KeyCode> {
        let s = match name {
            "forward" => &self.forward,
            "backward" => &self.backward,
            "left" => &self.left,
            "right" => &self.right,
            "strafe_left" => &self.strafe_left,
            "strafe_right" => &self.strafe_right,
            "jump" => &self.jump,
            "run" => &self.run,
            _ => return None,
        };
        Self::parse_key(s)
    }

    pub fn parse_key(s: &str) -> Option<KeyCode> {
        match s {
            "KeyW" | "W" => Some(KeyCode::KeyW),
            "KeyA" | "A" => Some(KeyCode::KeyA),
            "KeyS" | "S" => Some(KeyCode::KeyS),
            "KeyD" | "D" => Some(KeyCode::KeyD),
            "KeyQ" | "Q" => Some(KeyCode::KeyQ),
            "KeyE" | "E" => Some(KeyCode::KeyE),
            "Space" => Some(KeyCode::Space),
            "ShiftLeft" => Some(KeyCode::ShiftLeft),
            "ShiftRight" => Some(KeyCode::ShiftRight),
            _ => None,
        }
    }
}

// -----------------------
// Animation policy schema
// -----------------------

#[derive(Deserialize, Debug, Clone)]
pub struct AnimationPolicy {
    /// Base locomotion clips used when no override is active.
    pub base: BaseAnimations,

    /// Optional semantic aliases (e.g. "dance" -> "Dance_Loop").
    #[serde(default)]
    pub clips: HashMap<String, String>,

    /// Data-defined overrides / abilities.
    #[serde(default)]
    pub overrides: Vec<AnimationOverrideDef>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BaseAnimations {
    pub idle: String,
    pub walk: String,
    pub run: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AnimationOverrideDef {
    /// Semantic ID used by PlayAnimation("<id>")
    pub id: String,

    /// The actual glTF animation clip name (e.g. "Sitting_Idle_Loop")
    pub clip: String,

    /// Higher priority overrides lower priority.
    #[serde(default = "default_priority")]
    pub priority: i32,

    /// If true, any movement cancels this override.
    #[serde(default)]
    pub cancel_on_move: bool,

    /// Optional stop command that cancels this override
    /// (e.g. PlayAnimation("stand") cancels "sit").
    #[serde(default)]
    pub stop_action: Option<String>,

    /// Whether this should loop.
    #[serde(default = "default_looping")]
    pub looping: bool,

    /// Optional duration (seconds) for one-shot overrides.
    /// If set, the override will auto-expire after this duration.
    #[serde(default)]
    pub duration: Option<f32>,
}

fn default_priority() -> i32 {
    0
}

fn default_looping() -> bool {
    true
}

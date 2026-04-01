use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug, Clone)]
pub struct PlayerConfig {
    pub model_path: String,
    pub initial_position: (f32, f32, f32),
    pub camera: CameraConfig,
    pub inputs: InputMap,

    /// Path to the animation policy file, relative to the project root.
    /// e.g. "prefabs/animation/player_policy.ron"
    pub animation_policy: String,
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
            // Letters
            "KeyA" | "A" => Some(KeyCode::KeyA),
            "KeyB" | "B" => Some(KeyCode::KeyB),
            "KeyC" | "C" => Some(KeyCode::KeyC),
            "KeyD" | "D" => Some(KeyCode::KeyD),
            "KeyE" | "E" => Some(KeyCode::KeyE),
            "KeyF" | "F" => Some(KeyCode::KeyF),
            "KeyG" | "G" => Some(KeyCode::KeyG),
            "KeyH" | "H" => Some(KeyCode::KeyH),
            "KeyI" | "I" => Some(KeyCode::KeyI),
            "KeyJ" | "J" => Some(KeyCode::KeyJ),
            "KeyK" | "K" => Some(KeyCode::KeyK),
            "KeyL" | "L" => Some(KeyCode::KeyL),
            "KeyM" | "M" => Some(KeyCode::KeyM),
            "KeyN" | "N" => Some(KeyCode::KeyN),
            "KeyO" | "O" => Some(KeyCode::KeyO),
            "KeyP" | "P" => Some(KeyCode::KeyP),
            "KeyQ" | "Q" => Some(KeyCode::KeyQ),
            "KeyR" | "R" => Some(KeyCode::KeyR),
            "KeyS" | "S" => Some(KeyCode::KeyS),
            "KeyT" | "T" => Some(KeyCode::KeyT),
            "KeyU" | "U" => Some(KeyCode::KeyU),
            "KeyV" | "V" => Some(KeyCode::KeyV),
            "KeyW" | "W" => Some(KeyCode::KeyW),
            "KeyX" | "X" => Some(KeyCode::KeyX),
            "KeyY" | "Y" => Some(KeyCode::KeyY),
            "KeyZ" | "Z" => Some(KeyCode::KeyZ),
            // Digits
            "Digit0" | "0" => Some(KeyCode::Digit0),
            "Digit1" | "1" => Some(KeyCode::Digit1),
            "Digit2" | "2" => Some(KeyCode::Digit2),
            "Digit3" | "3" => Some(KeyCode::Digit3),
            "Digit4" | "4" => Some(KeyCode::Digit4),
            "Digit5" | "5" => Some(KeyCode::Digit5),
            "Digit6" | "6" => Some(KeyCode::Digit6),
            "Digit7" | "7" => Some(KeyCode::Digit7),
            "Digit8" | "8" => Some(KeyCode::Digit8),
            "Digit9" | "9" => Some(KeyCode::Digit9),
            // Function keys
            "F1"  => Some(KeyCode::F1),
            "F2"  => Some(KeyCode::F2),
            "F3"  => Some(KeyCode::F3),
            "F4"  => Some(KeyCode::F4),
            "F5"  => Some(KeyCode::F5),
            "F6"  => Some(KeyCode::F6),
            "F7"  => Some(KeyCode::F7),
            "F8"  => Some(KeyCode::F8),
            "F9"  => Some(KeyCode::F9),
            "F10" => Some(KeyCode::F10),
            "F11" => Some(KeyCode::F11),
            "F12" => Some(KeyCode::F12),
            // Modifiers
            "ShiftLeft"   => Some(KeyCode::ShiftLeft),
            "ShiftRight"  => Some(KeyCode::ShiftRight),
            "ControlLeft" => Some(KeyCode::ControlLeft),
            "ControlRight"=> Some(KeyCode::ControlRight),
            "AltLeft"     => Some(KeyCode::AltLeft),
            "AltRight"    => Some(KeyCode::AltRight),
            // Common
            "Space"     => Some(KeyCode::Space),
            "Escape"    => Some(KeyCode::Escape),
            "Enter"     => Some(KeyCode::Enter),
            "Tab"       => Some(KeyCode::Tab),
            "Backspace" => Some(KeyCode::Backspace),
            "Delete"    => Some(KeyCode::Delete),
            // Arrow keys
            "ArrowUp"    => Some(KeyCode::ArrowUp),
            "ArrowDown"  => Some(KeyCode::ArrowDown),
            "ArrowLeft"  => Some(KeyCode::ArrowLeft),
            "ArrowRight" => Some(KeyCode::ArrowRight),
            _ => None,
        }
    }
}

// -----------------------
// Animation policy schema
// -----------------------

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct AnimationPolicy {
    /// Base locomotion clips used when no override is active.
    pub base: BaseAnimations,

    /// Optional semantic aliases (e.g. "dance" -> "Dance_Loop").
    #[serde(default)]
    pub clips: HashMap<String, String>,

    /// Data-defined overrides / abilities.
    #[serde(default)]
    pub overrides: Vec<AnimationOverrideDef>,

    /// Default transition duration (milliseconds) when switching animations.
    /// If omitted, transitions are instant.
    #[serde(default)]
    pub default_transition_ms: Option<u64>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BaseAnimations {
    pub idle: String,
    pub walk: String,
    pub run: String,
    pub jump_loop: String,
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

    /// Per-override transition duration (ms). If set, overrides the global default.
    #[serde(default)]
    pub transition_ms: Option<u64>,
}

fn default_priority() -> i32 {
    0
}

fn default_looping() -> bool {
    true
}

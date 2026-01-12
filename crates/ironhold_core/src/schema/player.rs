use bevy::prelude::*;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct PlayerConfig {
    pub model_path: String,
    pub initial_position: (f32, f32, f32),
    pub camera: CameraConfig,
    pub inputs: InputMap,
    pub animations: AnimationMap,
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

#[derive(Deserialize, Debug, Clone)]
pub struct AnimationMap {
    pub idle: String,
    pub walk: String,
    pub run: String,
    pub jump_enter: String,
    pub jump_loop: String,
    pub jump_exit: String,
    pub death: String,
    pub dance: String,
    pub crouch_idle: String,
    pub crouch_forward: String,
    pub roll: String,
}

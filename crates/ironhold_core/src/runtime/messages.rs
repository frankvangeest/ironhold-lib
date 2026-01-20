use bevy::prelude::*;

#[derive(Message, Debug, Clone)]
pub enum UiMessage {
    ButtonPressed(String),
}

#[derive(Message, Debug, Clone)]
pub enum SceneEvent {
    Requested(String),
    Loaded(String),
    Ready(String),
}

#[derive(Message, Debug, Clone)]
pub enum InputAction {
    Move(Vec2),
    Turn(f32),
    Look(Vec2),
    Jump(bool),
    Run(bool),
}

#[derive(Message, Debug, Clone)]
pub struct InputActionMessage {
    pub entity: Entity,
    pub action: InputAction,
}

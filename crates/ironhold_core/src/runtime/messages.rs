use bevy::prelude::*;

#[derive(Message, Debug, Clone)]
pub enum UiEvent {
    ButtonPressed(String),
}

/// Gameplay events emitted by capabilities (physics sensors, timers, game logic).
/// Distinct from `UiEvent` (which is for UI widgets) and `SceneEvent` (scene lifecycle).
///
/// `Trigger(name)` fires a named event directly into the interpreter pipeline.
/// The `name` is used as-is as the rule key — no prefix is added — so the naming
/// convention carried in the string itself (`"entity.collected:coin_01"`,
/// `"zone.entered:checkpoint_1"`, etc.) acts as the namespace.
#[derive(Message, Debug, Clone)]
pub enum GameEvent {
    Trigger(String),
}

#[derive(Message, Debug, Clone)]
pub enum SceneEvent {
    Requested(String),
    Loaded(String),
    Ready(String),
    /// Emitted just before a full scene replace (not overlays). Rules can react via
    /// `scene.unloading:<name>` to clean up (e.g. stop music, save state).
    Unloading(String),
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

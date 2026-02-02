use bevy::prelude::*;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub enum UiElement {
    Button {
        text: String,
        action: UiAction,
        #[serde(default)]
        position: Option<(f32, f32)>,
    },
}

#[derive(Deserialize, Debug, Clone, Component)]
pub enum UiAction {
    Trigger(String),
}

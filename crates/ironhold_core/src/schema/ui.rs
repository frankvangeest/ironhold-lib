use bevy::prelude::*;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub enum UiElement {
    Button {
        text: String,
        action: UiAction,
    },
}

#[derive(Deserialize, Debug, Clone, Component)]
pub enum UiAction {
    LoadScene(String),
}

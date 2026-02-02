use bevy::prelude::*;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub enum UiElement {
    Button {
        text: String,
        action: UiAction,
        #[serde(default)]
        position: Option<(f32, f32)>,
        #[serde(default)]
        width: Option<f32>,
        #[serde(default)]
        height: Option<f32>,
        #[serde(default)]
        font_size: Option<f32>,
        #[serde(default)]
        border_color: Option<(f32, f32, f32, f32)>,
        #[serde(default)]
        background_color: Option<(f32, f32, f32, f32)>,
        #[serde(default)]
        text_color: Option<(f32, f32, f32, f32)>,
    },
}

#[derive(Deserialize, Debug, Clone, Component)]
pub enum UiAction {
    Trigger(String),
}

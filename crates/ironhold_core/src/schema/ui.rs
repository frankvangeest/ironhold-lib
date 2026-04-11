use bevy::prelude::*;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, Component)]
pub enum UiAction {
    Trigger(String),
}

use bevy::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub enum Action {
    LoadScene(String),
    Quit,
    Log(String),
    Spawn(String),
    PlayAnimation(String),
}

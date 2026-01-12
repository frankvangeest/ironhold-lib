use bevy::prelude::*;

#[derive(Message, Debug, Clone)]
pub enum UiMessage {
    ButtonPressed(String), // The path to load or identifier
}

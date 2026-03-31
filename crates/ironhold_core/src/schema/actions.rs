use bevy::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub enum Action {
    LoadScene(String),
    Quit,
    Log(String),
    /// Spawn a prefab by ID. `id` is an optional stable handle for later Despawn;
    /// if omitted a unique one is generated automatically.
    Spawn {
        prefab: String,
        #[serde(default)]
        id: Option<String>,
    },
    /// Despawn a previously spawned entity by the ID used in Spawn.
    Despawn(String),
    PlayAnimation(String),
    PlaySound(String),
}

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
    /// Play an audio file in a loop as background music. Stops any currently playing music.
    PlayMusicLoop(String),
    /// Set global audio volume. Value is 0–100 (percent). 0 = mute, 100 = full.
    SetVolume(u8),
}

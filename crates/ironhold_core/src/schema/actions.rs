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
    /// Stop the currently playing background music.
    StopMusic,
    /// Load a scene on top of the current one without unloading the game world.
    /// Only the UI section of the overlay scene is spawned; 3D entities are ignored.
    /// Entities are tagged OverlayEntity and removed by UnloadOverlay or any full LoadScene.
    LoadSceneOverlay(String),
    /// Remove all OverlayEntity entities (dismiss the current overlay).
    UnloadOverlay,
    /// If an overlay is currently active: unload it. Otherwise: load the given path as an overlay.
    /// Use this for ESC-style toggles so the same key/button opens and closes the overlay.
    ToggleOverlay(String),
    /// Set global audio volume. Value is 0–100 (percent). 0 = mute, 100 = full.
    SetVolume(u8),
    /// Pre-load a scene asset into the cache so it's ready instantly when first needed.
    /// Takes a project-relative path. Does not spawn or transition; purely warms the cache.
    Preload(String),
    /// Transition the interpreter to a named logic state.
    /// Rules with a matching `when` field become active; rules in other states are suppressed.
    /// Use an empty string `""` to return to the stateless (always-fire) default.
    EnterState(String),
}

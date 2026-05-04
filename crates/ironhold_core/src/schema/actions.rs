use bevy::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum Action {
    LoadScene(String),
    Quit,
    Log(String),
    /// Spawn a prefab by ID.
    /// - `id` — optional stable handle for later `Despawn`; auto-generated if omitted.
    /// - `position` — explicit world-space position `(x, y, z)`; takes precedence over `spawn_point`.
    /// - `spawn_point` — name of a spawn point defined in the scene's `spawn_points` map.
    ///   If neither `position` nor `spawn_point` is given, the entity spawns at the world origin.
    /// - `yaw_deg` — optional Y-axis rotation in degrees (0 = model default facing, 90 = 90° clockwise).
    ///   Covers N/S/E/W compass orientations. Defaults to 0 if omitted.
    Spawn {
        prefab: String,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        position: Option<(f32, f32, f32)>,
        #[serde(default)]
        spawn_point: Option<String>,
        #[serde(default)]
        yaw_deg: Option<f32>,
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
    /// Takes a project-relative path to a `.scene.ron`. Does not spawn or transition; purely
    /// warms the cache so a subsequent `LoadScene` resolves instantly.
    PreloadScene(String),
    /// Pre-load a prefab's GLB model so the first `Spawn` of that prefab doesn't block the
    /// game loop with asset decode on the WASM main thread. Takes a prefab key (as defined in
    /// `prefabs.ron`). Fire on `scene.ready:{name}` so the GLB is warm before the player
    /// can trigger a spawn. Does not create any visible entity.
    PreloadPrefab(String),
    /// Transition the interpreter to a named logic state.
    /// Rules with a matching `when` field become active; rules in other states are suppressed.
    /// Use an empty string `""` to return to the stateless (always-fire) default.
    EnterState(String),
    /// Set a named runtime variable to a string value.
    /// The value is stored in `GameVariables` and readable by data-bound UI labels.
    /// Example: `SetVariable("level", "2")` or `SetVariable("player_name", "Hero")`.
    SetVariable(String, String),
    /// Add (or subtract if negative) a numeric delta to a named variable.
    /// The variable is parsed as `i32`; missing or unparseable values default to `0`.
    /// Example: `IncrementVariable("score", 10)` awards 10 points;
    ///          `IncrementVariable("score", -5)` deducts 5.
    IncrementVariable(String, i32),
    /// Play an animation clip on a specific entity identified by its spawn ID.
    /// Use `target: "{self}"` inside behavior files — the entity FSM interpreter
    /// substitutes `{self}` with the entity's spawn ID before queuing the action.
    PlayAnimationOn {
        /// Spawn ID of the target entity, or `"{self}"` inside behavior files.
        target: String,
        /// Name of the animation clip to play.
        clip: String,
    },
    /// Emit a `GameEvent::Trigger` with the given name.
    /// Inside behavior files, `{self}` in the event name is replaced with the entity's
    /// spawn ID before the event is written, allowing reusable behavior-driven signals.
    EmitEvent(String),
}

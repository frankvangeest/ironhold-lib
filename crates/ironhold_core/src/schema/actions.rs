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
    /// Play a one-shot sound effect by catalog key.
    /// `volume` (0.0–1.0) multiplies the entry's catalog volume. Defaults to 1.0 when omitted.
    PlaySound {
        key: String,
        #[serde(default = "default_action_volume")]
        volume: f32,
    },
    /// Play an audio file in a loop as background music. Stops any currently playing music.
    /// `volume` (0.0–1.0) multiplies the entry's catalog volume. Defaults to 1.0 when omitted.
    PlayMusicLoop {
        key: String,
        #[serde(default = "default_action_volume")]
        volume: f32,
    },
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
    /// Add `delta` to a named stat (defined in `stats.ron`). Clamps to `[min, max]`.
    /// Negative delta reduces the stat and resets the regen cooldown.
    /// Example: `ModifyStat(key: "health", delta: -25.0)`.
    ModifyStat {
        key: String,
        delta: f32,
    },
    /// Set a named stat to an absolute value (defined in `stats.ron`). Clamps to `[min, max]`.
    /// Example: `SetStat(key: "health", value: 100.0)`.
    SetStat {
        key: String,
        value: f32,
    },
    /// Apply a named modifier template (defined in `stats.ron`) to its target stat.
    /// Multiple applications stack according to the modifier's `stack_rule`.
    /// Timed modifiers expire automatically; permanent ones persist until `RemoveModifier`.
    /// Example: `ApplyModifier(modifier_key: "speed_boost")`.
    ApplyModifier {
        modifier_key: String,
    },
    /// Remove all active instances of a named modifier from its target stat.
    /// Emits `stat.modifier.removed:{modifier_key}` when at least one instance was removed.
    /// No-op if the modifier is not currently active.
    /// Example: `RemoveModifier(modifier_key: "poison")`.
    RemoveModifier {
        modifier_key: String,
    },
    /// Spawn a floating damage number above a named entity.
    /// Positive `amount` renders in green (healing); negative in red (damage).
    /// The number rises ~1.5 m over 1.2 s then despawns automatically.
    /// Inside behavior files, `{self}` in `entity` is resolved to the entity's spawn ID.
    /// Example: `ShowDamagePopup(entity: "{self}", amount: -25.0)`.
    ShowDamagePopup {
        entity: String,
        amount: f32,
    },
    /// Show or hide a spawned entity by its ID.
    /// `visible: true` restores the entity; `visible: false` hides it (entity remains in ECS).
    /// World labels (health bars, stat labels) tracking the entity are hidden automatically.
    /// Inside behavior files, `{self}` in `entity` is resolved to the entity's spawn ID.
    /// Example: `SetEntityVisible(entity: "{self}", visible: false)`.
    SetEntityVisible {
        entity: String,
        visible: bool,
    },
    /// Emit a `GameEvent::Trigger` with the given name after a delay (in seconds).
    /// The event is buffered in `DelayedEventQueue` and fired by `tick_delayed_events_system`.
    /// Cleared on `Action::LoadScene` so no stale events fire after a scene transition.
    /// Inside behavior files, `{self}` in `event` is resolved to the entity's spawn ID.
    /// Example: `EmitEventAfterDelay(event: "entity.respawning:{self}", delay_secs: 15.0)`.
    EmitEventAfterDelay {
        event: String,
        delay_secs: f32,
    },
}

fn default_action_volume() -> f32 { 1.0 }

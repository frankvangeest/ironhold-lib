use bevy::prelude::*;
use bevy::input::mouse::MouseButton;
use serde::Deserialize;
use std::collections::HashMap;
use crate::schema::catalog::MovementConfig;

#[derive(Deserialize, Debug, Clone)]
pub struct PlayerConfig {
    pub model_path: String,
    pub initial_position: (f32, f32, f32),
    pub camera: CameraConfig,
    pub inputs: InputMap,

    /// Path to the animation policy file, relative to the project root.
    /// e.g. "prefabs/animation/player_policy.ron"
    /// When absent, no animation system is attached to the player.
    #[serde(default)]
    pub animation_policy: Option<String>,

    /// Movement tuning read from `prefab.components.movement`.
    #[serde(default)]
    pub movement: MovementConfig,

    /// Scene entity id (e.g. `"player_01"`) — set by the scene loader so the player gets a
    /// `SpawnId` + `SpawnRegistry` entry like every other entity (enables id-targeted actions
    /// such as `ShowDamagePopup(entity: "player_01")`). Defaults empty for any RON-loaded use.
    #[serde(default)]
    pub spawn_id: String,
    /// Prefab catalog key (e.g. `"player_warrior"`) — set by the scene loader for `PrefabKey`.
    #[serde(default)]
    pub prefab_key: String,
    /// Resolved display name for the player's nameplate widget. `None` = no nameplate.
    #[serde(default)]
    pub nameplate_display_name: Option<String>,
    /// `PrefabDef.nameplate` override forwarded to `NameplateTag`.
    #[serde(default)]
    pub nameplate_override: Option<bool>,
    /// Forwarded from `PrefabDef.player_index`. Distinguishes local co-op players (P1/P2/...)
    /// from a single-player scene, where it stays `0` and is unused.
    #[serde(default)]
    pub player_index: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CameraConfig {
    pub offset: (f32, f32, f32),
    pub look_at_offset: (f32, f32, f32),
    pub zoom_speed: f32,
    pub orbit_speed: f32,
    pub min_radius: f32,
    pub max_radius: f32,
    /// Minimum pitch in radians (looking up). Default: 0.1.
    #[serde(default = "default_min_pitch")]
    pub min_pitch: f32,
    /// Maximum pitch in radians (looking down). Default: 0.9.
    #[serde(default = "default_max_pitch")]
    pub max_pitch: f32,
    /// Mouse button that orbits the camera. `"Left"`, `"Right"`, or `"Either"`. Default: `"Either"`.
    #[serde(default = "default_orbit_button")]
    pub orbit_button: String,
    /// Mouse button that also rotates the character yaw while orbiting.
    /// `"Left"`, `"Right"`, or `None`. Default: `"Right"`.
    #[serde(default = "default_character_rotate_button")]
    pub character_rotate_button: Option<String>,
    /// Camera pitch at scene start in radians. Default: 0.5.
    #[serde(default = "default_initial_pitch")]
    pub initial_pitch: f32,
    /// Camera yaw at scene start in radians. Default: 0.0.
    #[serde(default)]
    pub initial_yaw: f32,
    /// Local co-op only: when the scene has 2+ `tags: ["player"]` entities, this player's
    /// `party` block (read from the *first* player only — later players' `party` fields are
    /// ignored) is the sole switch that spawns a single shared `PartyOrbitCamera` framing all
    /// players instead of each getting their own `OrbitCamera`. Absent on a 2+ player scene
    /// logs a warning and falls back to a single-player camera on the first player only —
    /// never silently spawns competing per-player cameras. Meaningless for single-player scenes.
    /// Mutually exclusive with `split` — if both are set on the first player, `split` wins and a
    /// warning is logged.
    #[serde(default)]
    pub party: Option<PartyZoomDef>,
    /// Local co-op only: like `party`, read from the *first* player only. When set (and 2+
    /// players are present), spawns one real `OrbitCamera` per player, each rendering to its own
    /// share of the window (`SplitScreenDef.orientation`) instead of a single shared
    /// `PartyOrbitCamera`. Mutually exclusive with `party` — if both are set, `split` wins and a
    /// warning is logged.
    #[serde(default)]
    pub split: Option<SplitScreenDef>,
}

/// Local co-op shared-camera zoom behavior, authored on the first player's `camera.party`.
#[derive(Deserialize, Debug, Clone)]
pub struct PartyZoomDef {
    /// Extra distance added beyond the raw max pairwise distance between players, so they
    /// aren't framed edge-to-edge.
    pub zoom_margin: f32,
    /// Whether manual scroll-zoom still nudges the derived radius (as an offset on top of the
    /// distance-driven value, not a replacement for it). Default: `false` — radius is fully
    /// derived from player separation, matching "camera zooms based on player distance" with
    /// no player-controlled override fighting it.
    #[serde(default)]
    pub allow_manual_zoom: bool,
}

/// Local co-op split-screen configuration, authored on the first player's `camera.split`.
#[derive(Deserialize, Debug, Clone)]
pub struct SplitScreenDef {
    /// The fixed split axis when `dynamic` is unset (Stage 3/4 behavior). When `dynamic` IS set,
    /// this instead becomes a rare tie-break hint — used only on the exact frame the two players
    /// are equally separated on both axes — since the live split axis is otherwise chosen
    /// automatically from their actual relative position. Optional because dynamic-mode authors
    /// usually don't need to think about it; defaults to `Vertical`.
    #[serde(default)]
    pub orientation: SplitOrientation,
    /// Local co-op only: when set, the screen starts merged into a single shared camera (framing
    /// both players like `party`) and automatically splits into two independent per-player
    /// cameras once they separate beyond `split_distance`, merging back below `merge_distance`.
    /// See `DynamicSplitDef`.
    #[serde(default)]
    pub dynamic: Option<DynamicSplitDef>,
}

/// How the window is divided between local co-op players' individual cameras.
/// `Vertical` and `Horizontal` are both implemented; when `SplitScreenDef.dynamic` is set, the
/// live split axis is chosen automatically instead (see `SplitScreenDef.orientation`'s doc).
#[derive(Deserialize, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SplitOrientation {
    /// Left half / right half, split down the middle.
    #[default]
    Vertical,
    /// Top half / bottom half, split down the middle.
    Horizontal,
}

/// Local co-op dynamic split-screen tuning, authored on the first player's `camera.split.dynamic`.
/// Self-contained rather than reusing `party:` — `party` and `split` are mutually exclusive
/// elsewhere in this schema, and requiring both together just for dynamic mode would complicate
/// that rule. `merged_zoom_margin`/`merged_allow_manual_zoom` mirror `PartyZoomDef`'s fields
/// exactly; dynamic mode spawns its own internal `PartyOrbitCamera` for the merged state using
/// them, with no `party:` block required.
#[derive(Deserialize, Debug, Clone)]
pub struct DynamicSplitDef {
    /// Distance beyond which the merged camera splits into two independent per-player cameras.
    /// No default — the right value depends on room size and player `walk_speed`, so it must be
    /// authored per-scene.
    pub split_distance: f32,
    /// Distance below which a split view merges back into one shared camera. Must be less than
    /// `split_distance` (the gap prevents flicker right at either boundary) — if authored
    /// backwards, a warning is logged and it's clamped just below `split_distance`. No default,
    /// same reasoning as `split_distance`.
    pub merge_distance: f32,
    /// Extra distance added beyond the raw pairwise distance between players while merged —
    /// same meaning as `PartyZoomDef.zoom_margin`.
    pub merged_zoom_margin: f32,
    /// Whether manual scroll-zoom still nudges the merged camera's derived radius — same meaning
    /// as `PartyZoomDef.allow_manual_zoom`. Default: `false`.
    #[serde(default)]
    pub merged_allow_manual_zoom: bool,
}

fn default_min_pitch() -> f32 { 0.1 }
fn default_max_pitch() -> f32 { 0.9 }
fn default_orbit_button() -> String { "Either".to_string() }
fn default_character_rotate_button() -> Option<String> { Some("Right".to_string()) }
fn default_initial_pitch() -> f32 { 0.5 }

#[derive(Deserialize, Debug, Clone)]
pub struct InputMap {
    pub forward: String,
    pub backward: String,
    pub left: String,
    pub right: String,
    pub strafe_left: String,
    pub strafe_right: String,
    pub jump: String,
    #[serde(default = "default_run_key")]
    pub run: String,
    #[serde(default = "default_interact_key")]
    pub interact: String,
    /// Mouse button that enables strafe-mode (A/D strafe instead of rotate).
    /// `"Left"`, `"Right"`, or `None` to disable mouse-strafe entirely.
    /// Default: `"Left"` (preserves existing behavior).
    #[serde(default = "default_strafe_mouse_button")]
    pub strafe_mouse_button: Option<String>,
    /// Key to cycle to the next nearest `targetable: true` entity (default: `"Tab"`).
    /// Hold Shift while pressing to cycle in reverse (nearest-last).
    #[serde(default = "default_target_next_key")]
    pub target_next: String,
    /// Maximum world-space distance (in units) to consider for Tab targeting (default: 30.0).
    #[serde(default = "default_target_range")]
    pub target_range: f32,
    /// When set, this player reads input from the connected gamepad at this index instead of
    /// the keyboard — lets local co-op scenes bind player 2 to a controller. `None` (default)
    /// keeps keyboard-only behavior identical to before this field existed.
    #[serde(default)]
    pub gamepad_index: Option<usize>,
}

fn default_run_key() -> String {
    "ShiftLeft".to_string()
}

fn default_interact_key() -> String {
    "KeyF".to_string()
}

fn default_strafe_mouse_button() -> Option<String> { Some("Left".to_string()) }
fn default_target_next_key() -> String { "Tab".to_string() }
fn default_target_range() -> f32 { 30.0 }

impl InputMap {
    pub fn parse_mouse_button(s: &str) -> Option<MouseButton> {
        match s {
            "Left"   => Some(MouseButton::Left),
            "Right"  => Some(MouseButton::Right),
            "Middle" => Some(MouseButton::Middle),
            _        => None,
        }
    }

    pub fn key(&self, name: &str) -> Option<KeyCode> {
        let s = match name {
            "forward" => &self.forward,
            "backward" => &self.backward,
            "left" => &self.left,
            "right" => &self.right,
            "strafe_left" => &self.strafe_left,
            "strafe_right" => &self.strafe_right,
            "jump" => &self.jump,
            "run" => &self.run,
            "interact" => &self.interact,
            _ => return None,
        };
        Self::parse_key(s)
    }

    pub fn parse_key(s: &str) -> Option<KeyCode> {
        match s {
            // Letters
            "KeyA" | "A" => Some(KeyCode::KeyA),
            "KeyB" | "B" => Some(KeyCode::KeyB),
            "KeyC" | "C" => Some(KeyCode::KeyC),
            "KeyD" | "D" => Some(KeyCode::KeyD),
            "KeyE" | "E" => Some(KeyCode::KeyE),
            "KeyF" | "F" => Some(KeyCode::KeyF),
            "KeyG" | "G" => Some(KeyCode::KeyG),
            "KeyH" | "H" => Some(KeyCode::KeyH),
            "KeyI" | "I" => Some(KeyCode::KeyI),
            "KeyJ" | "J" => Some(KeyCode::KeyJ),
            "KeyK" | "K" => Some(KeyCode::KeyK),
            "KeyL" | "L" => Some(KeyCode::KeyL),
            "KeyM" | "M" => Some(KeyCode::KeyM),
            "KeyN" | "N" => Some(KeyCode::KeyN),
            "KeyO" | "O" => Some(KeyCode::KeyO),
            "KeyP" | "P" => Some(KeyCode::KeyP),
            "KeyQ" | "Q" => Some(KeyCode::KeyQ),
            "KeyR" | "R" => Some(KeyCode::KeyR),
            "KeyS" | "S" => Some(KeyCode::KeyS),
            "KeyT" | "T" => Some(KeyCode::KeyT),
            "KeyU" | "U" => Some(KeyCode::KeyU),
            "KeyV" | "V" => Some(KeyCode::KeyV),
            "KeyW" | "W" => Some(KeyCode::KeyW),
            "KeyX" | "X" => Some(KeyCode::KeyX),
            "KeyY" | "Y" => Some(KeyCode::KeyY),
            "KeyZ" | "Z" => Some(KeyCode::KeyZ),
            // Digits
            "Digit0" | "0" => Some(KeyCode::Digit0),
            "Digit1" | "1" => Some(KeyCode::Digit1),
            "Digit2" | "2" => Some(KeyCode::Digit2),
            "Digit3" | "3" => Some(KeyCode::Digit3),
            "Digit4" | "4" => Some(KeyCode::Digit4),
            "Digit5" | "5" => Some(KeyCode::Digit5),
            "Digit6" | "6" => Some(KeyCode::Digit6),
            "Digit7" | "7" => Some(KeyCode::Digit7),
            "Digit8" | "8" => Some(KeyCode::Digit8),
            "Digit9" | "9" => Some(KeyCode::Digit9),
            // Function keys
            "F1"  => Some(KeyCode::F1),
            "F2"  => Some(KeyCode::F2),
            "F3"  => Some(KeyCode::F3),
            "F4"  => Some(KeyCode::F4),
            "F5"  => Some(KeyCode::F5),
            "F6"  => Some(KeyCode::F6),
            "F7"  => Some(KeyCode::F7),
            "F8"  => Some(KeyCode::F8),
            "F9"  => Some(KeyCode::F9),
            "F10" => Some(KeyCode::F10),
            "F11" => Some(KeyCode::F11),
            "F12" => Some(KeyCode::F12),
            // Modifiers
            "ShiftLeft"   => Some(KeyCode::ShiftLeft),
            "ShiftRight"  => Some(KeyCode::ShiftRight),
            "ControlLeft" => Some(KeyCode::ControlLeft),
            "ControlRight"=> Some(KeyCode::ControlRight),
            "AltLeft"     => Some(KeyCode::AltLeft),
            "AltRight"    => Some(KeyCode::AltRight),
            // Common
            "Space"     => Some(KeyCode::Space),
            "Escape"    => Some(KeyCode::Escape),
            "Enter"     => Some(KeyCode::Enter),
            "Tab"       => Some(KeyCode::Tab),
            "Backspace" => Some(KeyCode::Backspace),
            "Delete"    => Some(KeyCode::Delete),
            // Arrow keys
            "ArrowUp"    => Some(KeyCode::ArrowUp),
            "ArrowDown"  => Some(KeyCode::ArrowDown),
            "ArrowLeft"  => Some(KeyCode::ArrowLeft),
            "ArrowRight" => Some(KeyCode::ArrowRight),
            _ => None,
        }
    }
}

// -----------------------
// Animation policy schema
// -----------------------

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct AnimationPolicy {
    /// Base locomotion clips used when no override is active.
    pub base: BaseAnimations,

    /// Optional semantic aliases (e.g. "dance" -> "Dance_Loop").
    #[serde(default)]
    pub clips: HashMap<String, String>,

    /// Data-defined overrides / abilities.
    #[serde(default)]
    pub overrides: Vec<AnimationOverrideDef>,

    /// Default transition duration (milliseconds) when switching animations.
    /// If omitted, transitions are instant.
    #[serde(default)]
    pub default_transition_ms: Option<u64>,

    /// Extra GLB catalog keys whose `named_animations` are merged into this character's
    /// animation graph alongside the model GLB's own clips.
    /// All listed GLBs must share the same bone names as the model GLB.
    /// Last entry wins on duplicate clip names.
    #[serde(default)]
    pub animation_sources: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct BaseAnimations {
    pub idle: String,
    pub walk: String,
    pub run: String,
    pub jump_loop: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct AnimationOverrideDef {
    /// Semantic ID used by PlayAnimation("<id>")
    pub id: String,

    /// The actual glTF animation clip name (e.g. "Sitting_Idle_Loop")
    pub clip: String,

    /// Higher priority overrides lower priority.
    #[serde(default = "default_priority")]
    pub priority: i32,

    /// If true, any movement cancels this override.
    #[serde(default)]
    pub cancel_on_move: bool,

    /// Optional stop command that cancels this override
    /// (e.g. PlayAnimation("stand") cancels "sit").
    #[serde(default)]
    pub stop_action: Option<String>,

    /// Whether this should loop.
    #[serde(default = "default_looping")]
    pub looping: bool,

    /// Optional duration (seconds) for one-shot overrides.
    /// If set, the override will auto-expire after this duration.
    #[serde(default)]
    pub duration: Option<f32>,

    /// Per-override transition duration (ms). If set, overrides the global default.
    #[serde(default)]
    pub transition_ms: Option<u64>,
}

fn default_priority() -> i32 {
    0
}

fn default_looping() -> bool {
    true
}

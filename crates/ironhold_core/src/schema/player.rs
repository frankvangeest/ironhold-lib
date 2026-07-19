use bevy::prelude::*;
use bevy::input::mouse::MouseButton;
use serde::Deserialize;
use std::collections::HashMap;
use crate::schema::catalog::MovementConfig;

/// Where a player's visual body + physics collider come from. Dispatched on by
/// `spawn_player_entity_core` for body construction only — every other player-construction
/// concern (`PlayerIndex`, `StatMap`, material override, nameplate, stat widgets) is shared
/// unconditionally by both variants. See `planning/features/done/player_model_source_unification.md`.
#[derive(Debug, Clone)]
pub enum PlayerModelSource {
    /// A GLB model, resolved from the asset catalog. The `#`-fragment (if any) is the scene
    /// name within the glTF file — stripped by the loader before use.
    Glb(String),
    /// A primitive (capsule/etc.) shape built at spawn time — same construction path a
    /// `kind: Primitive` NPC/prop already uses, plus cosmetic `children`.
    Primitive {
        shape: crate::schema::catalog::PrimitiveShapeKind,
        params: crate::schema::catalog::PrimitiveParams,
        children: Vec<crate::schema::catalog::ChildPrimitiveDef>,
    },
}

// `PlayerConfig` is assembled programmatically (`assemble_player_config`), never deserialized
// directly from scene RON — so it doesn't need `Deserialize`, and `PlayerModelSource` (which
// can't derive it meaningfully anyway, since RON never authors this type) doesn't need it either.
#[derive(Debug, Clone)]
pub struct PlayerConfig {
    pub model_source: PlayerModelSource,
    pub initial_position: (f32, f32, f32),
    pub camera: CameraConfig,
    pub inputs: InputMap,

    /// Path to the animation policy file, relative to the project root.
    /// e.g. "prefabs/animation/player_policy.ron"
    /// When absent, no animation system is attached to the player.
    pub animation_policy: Option<String>,

    /// Movement tuning read from `prefab.components.movement`.
    pub movement: MovementConfig,

    /// Scene entity id (e.g. `"player_01"`) — set by the scene loader so the player gets a
    /// `SpawnId` + `SpawnRegistry` entry like every other entity (enables id-targeted actions
    /// such as `ShowDamagePopup(entity: "player_01")`). Defaults empty for any RON-loaded use.
    pub spawn_id: String,
    /// Prefab catalog key (e.g. `"player_warrior"`) — set by the scene loader for `PrefabKey`.
    pub prefab_key: String,
    /// Resolved display name for the player's nameplate widget. `None` = no nameplate.
    pub nameplate_display_name: Option<String>,
    /// `PrefabDef.nameplate` override forwarded to `NameplateTag`.
    pub nameplate_override: Option<bool>,
    /// Forwarded from `PrefabDef.player_index`. Distinguishes local co-op players (P1/P2/...)
    /// from a single-player scene, where it stays `0` and is unused.
    pub player_index: u32,
    /// Forwarded from `PrefabDef.material`. Applied via `PendingMaterialOverride` in
    /// `spawn_player_entity_core`, same mechanism as the generic Actor/Prop spawn path
    /// (`spawn_prefab_instance`) — players have their own dedicated spawn path that does not
    /// otherwise read `PrefabDef.material` at all.
    pub material: Option<String>,
    /// Forwarded from `PrefabDef.stat_templates`. When non-empty, `spawn_player_entity_core`
    /// inserts a `StatMap` component on the player entity (same mechanism NPCs/props already use
    /// via `attach_prefab_features`) — giving this player their own independent stat pool. Empty
    /// by default, so a player prefab with no `stat_templates` gets no `StatMap` and the action
    /// bar's `SlotCost` falls back to the global `LoadedStats` resource exactly as before this
    /// field existed. See `planning/features/per_player_stat_pools.md`.
    pub stat_templates: Vec<crate::schema::stats::StatTemplateDef>,
    /// Forwarded from `PrefabDef.stat_label`. When set, `spawn_player_entity_core` queues a
    /// floating stat-label widget for this player via `DynamicStatUiQueue`, the same mechanism
    /// NPC/prop `Action::Spawn` entities use — giving players first-class stat widgets instead
    /// of the field silently parsing and doing nothing. See
    /// `planning/features/player_stat_widgets.md`.
    pub stat_label: Option<crate::schema::catalog::StatLabelDef>,
    /// Forwarded from `PrefabDef.world_stat_bar`. Same mechanism as `stat_label` above.
    pub world_stat_bar: Option<crate::schema::catalog::WorldStatBarDef>,
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
    /// Angular rate (radians/sec) for keyboard-held camera look (`InputMap.look_left`/
    /// `look_right`/`look_up`/`look_down`). Deliberately NOT the same field as `orbit_speed`,
    /// which is tuned as a mouse-pixel-delta multiplier and would be far too slow
    /// (~15s/revolution at this scene's `orbit_speed: 0.4`) if reused as a keyboard-hold rate.
    /// Default: 2.0 (~3.1s per full yaw revolution held).
    #[serde(default = "default_look_speed")]
    pub look_speed: f32,
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
/// `Vertical`, `Horizontal`, and `Grid` are all implemented; when `SplitScreenDef.dynamic` is set,
/// the live split axis is chosen automatically between `Vertical`/`Horizontal` instead (see
/// `SplitScreenDef.orientation`'s doc) — `dynamic` does not support `Grid`.
#[derive(Deserialize, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SplitOrientation {
    /// Left half / right half, split down the middle. Always exactly 2-way.
    #[default]
    Vertical,
    /// Top half / bottom half, split down the middle. Always exactly 2-way.
    Horizontal,
    /// N-way grid (`cols = ceil(sqrt(count))`, `rows = ceil(count / cols)`), static only (no
    /// `dynamic` support). Player count is read from the scene's entity count at load, capped at
    /// `MAX_SPLIT_PLAYERS` — see `capabilities::camera::split_screen_viewport_system`. A count of
    /// 3 leaves one grid cell empty; more than `MAX_SPLIT_PLAYERS` players spawn cameraless.
    Grid,
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
fn default_look_speed() -> f32 { 2.0 }

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
    /// Keyboard-held camera yaw/pitch turning, independent of every other player's camera —
    /// needed because split-screen scenes disable mouse-orbit per camera (`orbit_button: "None"`,
    /// since one shared mouse can't drive 2+ independently-active `OrbitCamera`s). `None`
    /// (default) leaves that axis unbound; all four are optional and independent of each other.
    /// See `docs/20_data_formats.md`'s `InputMap` table for the "valid key name strings" this
    /// accepts (parsed the same way as `forward`/`jump`/etc., via `InputMap::parse_key`).
    #[serde(default)]
    pub look_left: Option<String>,
    #[serde(default)]
    pub look_right: Option<String>,
    #[serde(default)]
    pub look_up: Option<String>,
    #[serde(default)]
    pub look_down: Option<String>,
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
        // Normalize a single lowercase ASCII letter (e.g. "q" -> "Q") so the single-character
        // letter form is case-insensitive; multi-character key names (e.g. "Escape", "KeyQ")
        // stay case-sensitive as authored.
        let normalized;
        let s = if s.len() == 1 && s.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
            normalized = s.to_ascii_uppercase();
            normalized.as_str()
        } else {
            s
        };
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
            // Numpad (physical keys, unaffected by NumLock state)
            "Numpad0" => Some(KeyCode::Numpad0),
            "Numpad1" => Some(KeyCode::Numpad1),
            "Numpad2" => Some(KeyCode::Numpad2),
            "Numpad3" => Some(KeyCode::Numpad3),
            "Numpad4" => Some(KeyCode::Numpad4),
            "Numpad5" => Some(KeyCode::Numpad5),
            "Numpad6" => Some(KeyCode::Numpad6),
            "Numpad7" => Some(KeyCode::Numpad7),
            "Numpad8" => Some(KeyCode::Numpad8),
            "Numpad9" => Some(KeyCode::Numpad9),
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
            // Punctuation — added for the Arrows control scheme's camera-look bindings
            // (Comma/Period sit physically beside the arrow cluster on a standard layout); the
            // rest of the row is added alongside for a complete, non-arbitrary set.
            "Comma"        => Some(KeyCode::Comma),
            "Period"       => Some(KeyCode::Period),
            "Semicolon"    => Some(KeyCode::Semicolon),
            "Quote"        => Some(KeyCode::Quote),
            "Slash"        => Some(KeyCode::Slash),
            "BracketLeft"  => Some(KeyCode::BracketLeft),
            "BracketRight" => Some(KeyCode::BracketRight),
            "Minus"        => Some(KeyCode::Minus),
            "Equal"        => Some(KeyCode::Equal),
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

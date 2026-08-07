use serde::Deserialize;
use crate::schema::player::CameraConfig;
use crate::schema::catalog::FlyCamDef;

/// A named camera preset, authored under a prefab's `components.camera_mode`. Replaces the old
/// implicit "orbit if tagged player, flycam if tagged flycam" dispatch with an explicit,
/// designer-chosen mode. See `planning/features/camera_modes.md`.
///
/// This is the *authored* (`Deserialize`) schema shape only — it cannot hold `Entity` references,
/// pre-resolved `KeyCode`s, or mutable per-frame state (yaw/pitch/radius). The runtime-resolved
/// equivalent is `capabilities::camera::ActiveCameraMode`, built from this at spawn time exactly
/// like today's `OrbitCamera` is built from a `CameraConfig`.
///
/// `split:`/`party:` (local co-op viewport assignment) are NOT part of this enum — they live as
/// a sibling field of `camera_mode` under `components:` (`PrefabComponents::split`/`::party`),
/// since viewport assignment is orthogonal to which camera-following mode a player uses.
#[derive(Deserialize, Debug, Clone)]
pub enum CameraModeDef {
    /// Mouse/keyboard/gamepad-orbitable camera following a target at a variable radius.
    /// Reuses `CameraConfig` — the exact struct the legacy `camera:` field already uses.
    Orbit(CameraConfig),
    /// Fixed-offset follow camera, no free orbit — good for top-down/side-scrollers.
    Follow(FollowCameraDef),
    /// Camera locked to the target's head position; mouse/gamepad controls look direction.
    FirstPerson(FirstPersonCameraDef),
    /// Static camera at a world position, looking at a fixed point or a named tracked entity.
    Fixed(FixedCameraDef),
    /// Free-flying camera, keyboard + mouse look, no target. Reuses `FlyCamDef`.
    Flycam(FlyCamDef),
    /// Shared camera framing every local-coop player at once. Authored directly only when a
    /// designer wants an explicit standalone party camera outside the split-screen player-count
    /// dispatch in `spawn_players_and_camera` (which spawns its own internal `Party`-mode camera
    /// from the sibling `party:`/`split.dynamic` fields, not from this variant).
    Party(PartyCameraDef),
}

fn default_follow_smoothing() -> f32 { 8.0 }
fn default_follow_rotation_smoothing() -> f32 { 6.0 }
fn default_fov() -> f32 { 60.0 }

/// `camera_mode: Follow(...)` payload.
#[derive(Deserialize, Debug, Clone)]
pub struct FollowCameraDef {
    pub offset: (f32, f32, f32),
    #[serde(default)]
    pub look_at_offset: (f32, f32, f32),
    /// Position lerp speed — higher = snappier, 0 = instant. Default: 8.0.
    #[serde(default = "default_follow_smoothing")]
    pub smoothing: f32,
    /// Separate smoothing for look-at rotation. Default: 6.0.
    #[serde(default = "default_follow_rotation_smoothing")]
    pub rotation_smoothing: f32,
    /// Field of view in degrees. Default: 60.0.
    #[serde(default = "default_fov")]
    pub fov: f32,
}

fn default_first_person_sensitivity() -> f32 { 0.002 }
fn default_first_person_min_pitch() -> f32 { -1.4 }
fn default_first_person_max_pitch() -> f32 { 1.4 }
fn default_first_person_fov() -> f32 { 90.0 }

/// `camera_mode: FirstPerson(...)` payload.
#[derive(Deserialize, Debug, Clone)]
pub struct FirstPersonCameraDef {
    pub eye_offset: (f32, f32, f32),
    #[serde(default = "default_first_person_sensitivity")]
    pub sensitivity: f32,
    #[serde(default = "default_first_person_min_pitch")]
    pub min_pitch: f32,
    #[serde(default = "default_first_person_max_pitch")]
    pub max_pitch: f32,
    #[serde(default = "default_first_person_fov")]
    pub fov: f32,
}

/// `camera_mode: Fixed(...)` payload. Exactly one of `look_at`/`look_at_entity` should be set;
/// if both are set, `look_at_entity` wins (re-resolved every frame) and a warning is logged.
#[derive(Deserialize, Debug, Clone)]
pub struct FixedCameraDef {
    pub position: (f32, f32, f32),
    #[serde(default)]
    pub look_at: Option<(f32, f32, f32)>,
    /// Prefab instance id (scene entity id), re-resolved every frame so the camera keeps
    /// pointing at the target as it moves.
    #[serde(default)]
    pub look_at_entity: Option<String>,
    #[serde(default = "default_fov")]
    pub fov: f32,
}

fn default_party_orbit_button() -> String { "Right".to_string() }

/// `camera_mode: Party(...)` payload — a standalone, directly-authored party camera. Consolidates
/// fields that today are split across `CameraConfig` (min_radius/max_radius/orbit_speed/
/// zoom_speed/orbit_button) and `PartyZoomDef` (zoom_margin/allow_manual_zoom) into one struct,
/// since a directly-authored `Party` mode has no base `CameraConfig` to draw the former from.
#[derive(Deserialize, Debug, Clone)]
pub struct PartyCameraDef {
    #[serde(default)]
    pub look_at_offset: (f32, f32, f32),
    /// Extra distance added beyond the raw max pairwise distance between targets.
    pub zoom_margin: f32,
    pub min_radius: f32,
    pub max_radius: f32,
    #[serde(default)]
    pub orbit_speed: f32,
    #[serde(default)]
    pub zoom_speed: f32,
    #[serde(default = "default_party_orbit_button")]
    pub orbit_button: String,
    #[serde(default)]
    pub allow_manual_zoom: bool,
}

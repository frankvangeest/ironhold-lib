use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

/// Scene file format version 2.
#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct GameSceneV2 {
    pub schema_version: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub lighting: Option<SceneLightingV2>,
    #[serde(default)]
    pub terrain: Option<TerrainConfigV2>,
    #[serde(default)]
    pub spawn_points: HashMap<String, (f32, f32, f32)>,
    #[serde(default)]
    pub entities: Vec<SceneEntityDef>,
    #[serde(default)]
    pub ui: Vec<UiElementDefV2>,
    /// When set, the UI elements are laid out in a centered panel box instead of
    /// using absolute positioning. `position` on each element is ignored in this mode.
    #[serde(default)]
    pub ui_panel: Option<UiPanelDef>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SceneLightingV2 {
    #[serde(default)]
    pub ambient: Option<(f32, f32, f32)>,
    #[serde(default)]
    pub directional: Option<DirectionalLightDefV2>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DirectionalLightDefV2 {
    pub color: (f32, f32, f32),
    pub intensity: f32,
    pub rotation_euler_deg: (f32, f32, f32),
    #[serde(default = "default_true")]
    pub shadows_enabled: bool,
}

fn default_true() -> bool { true }

#[derive(Deserialize, Debug, Clone)]
pub struct TerrainConfigV2 {
    pub heightmap: String,
    pub splatmap: String,
    /// (horizontal_x, height_multiplier, horizontal_z) scale factors.
    pub scale: (f32, f32, f32),
    pub material_paths: Vec<String>,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u32,
}

fn default_chunk_size() -> u32 { 64 }

#[derive(Deserialize, Debug, Clone)]
pub struct SceneEntityDef {
    pub id: String,
    pub prefab: String,
    pub transform: SceneTransformV2,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct SceneTransformV2 {
    #[serde(default)]
    pub translation: (f32, f32, f32),
    #[serde(default)]
    pub rotation_euler_deg: (f32, f32, f32),
    #[serde(default = "one_vec3")]
    pub scale: (f32, f32, f32),
}

fn one_vec3() -> (f32, f32, f32) { (1.0, 1.0, 1.0) }

#[derive(Deserialize, Debug, Clone)]
pub struct UiElementDefV2 {
    /// "button" renders an interactive button; "label" renders non-interactive text.
    pub kind: String,
    pub id: String,
    pub text: String,
    /// Action trigger for kind="button". "ui." prefix is stripped when firing.
    /// May be omitted (defaults to empty) for kind="label".
    #[serde(default)]
    pub action: String,
    /// Absolute position in pixels. Ignored when `ui_panel` is set on the scene.
    #[serde(default)]
    pub position: (f32, f32),
    pub size: (f32, f32),
}

/// When present on a scene, UI elements are laid out in a centered panel box
/// instead of using absolute positioning.
#[derive(Deserialize, Debug, Clone)]
pub struct UiPanelDef {
    /// Background color of the panel box as RGBA (0.0–1.0).
    pub background_color: (f32, f32, f32, f32),
    /// Inner padding around panel contents in pixels.
    #[serde(default = "default_panel_padding")]
    pub padding: f32,
    /// Gap between child elements in pixels.
    #[serde(default = "default_panel_gap")]
    pub gap: f32,
    /// Optional fixed width of the panel in pixels. Auto-sized if omitted.
    #[serde(default)]
    pub width: Option<f32>,
}

fn default_panel_padding() -> f32 { 20.0 }
fn default_panel_gap() -> f32 { 12.0 }

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

pub const GAME_SCENE_V2_SCHEMA_VERSION: u32 = 2;

/// Scene file format version 2.
#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
#[serde(deny_unknown_fields)]
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

impl GameSceneV2 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != GAME_SCENE_V2_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported GameSceneV2 schema_version {} (expected {})",
                self.schema_version, GAME_SCENE_V2_SCHEMA_VERSION
            ));
        }
        let mut entity_ids = std::collections::HashSet::new();
        for entity in &self.entities {
            if entity.id.is_empty() {
                return Err("Scene entity has empty id".to_string());
            }
            if !entity_ids.insert(entity.id.as_str()) {
                return Err(format!("Duplicate scene entity id: \"{}\"", entity.id));
            }
        }
        let mut ui_ids = std::collections::HashSet::new();
        for elem in &self.ui {
            if elem.id.is_empty() {
                return Err("UI element has empty id".to_string());
            }
            if !ui_ids.insert(elem.id.as_str()) {
                return Err(format!("Duplicate UI element id: \"{}\"", elem.id));
            }
            if elem.kind != "button" && elem.kind != "label" {
                return Err(format!(
                    "UI element \"{}\" has unknown kind \"{}\" (expected \"button\" or \"label\")",
                    elem.id, elem.kind
                ));
            }
        }
        Ok(())
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct SceneLightingV2 {
    #[serde(default)]
    pub ambient: Option<(f32, f32, f32)>,
    /// Overrides the default ambient brightness (150.0).
    /// Maps to Bevy's `AmbientLight::brightness` (lux). Works with or without HDR;
    /// without HDR colours clip at 1.0, so keep values low (50–300 is typical).
    #[serde(default)]
    pub ambient_brightness: Option<f32>,
    #[serde(default)]
    pub directional: Option<DirectionalLightDefV2>,
    #[serde(default)]
    pub point_lights: Vec<PointLightDefV2>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DirectionalLightDefV2 {
    pub color: (f32, f32, f32),
    pub intensity: f32,
    pub rotation_euler_deg: (f32, f32, f32),
    #[serde(default = "default_true")]
    pub shadows_enabled: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PointLightDefV2 {
    pub position: (f32, f32, f32),
    #[serde(default = "default_point_color")]
    pub color: (f32, f32, f32),
    /// Luminous power in lumens (default: 800.0 ≈ a bright 60 W bulb).
    #[serde(default = "default_point_intensity")]
    pub intensity: f32,
    /// Radius of the sphere used for specular highlights (default: 0.0).
    #[serde(default)]
    pub radius: f32,
    /// Maximum range in world units (default: 20.0).
    #[serde(default = "default_point_range")]
    pub range: f32,
    #[serde(default)]
    pub shadows_enabled: bool,
}

fn default_true() -> bool { true }
fn default_point_color() -> (f32, f32, f32) { (1.0, 1.0, 1.0) }
fn default_point_intensity() -> f32 { 800.0 }
fn default_point_range() -> f32 { 20.0 }

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TerrainConfigV2 {
    pub heightmap: String,
    pub splatmap: String,
    /// `(horizontal, height, horizontal_z)` — world units per heightmap pixel (X/Z)
    /// and maximum terrain height in world units (Y).
    /// Example: `(5.0, 30.0, 5.0)` gives a 635×635 unit terrain with 30 units of
    /// elevation, using a 128×128 heightmap.
    pub scale: (f32, f32, f32),
    pub material_paths: Vec<String>,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u32,
    /// World-space offset applied to the entire terrain mesh.
    /// Defaults to the origin `(0, 0, 0)`.
    #[serde(default)]
    pub position: Option<(f32, f32, f32)>,
}

fn default_chunk_size() -> u32 { 64 }

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct SceneEntityDef {
    pub id: String,
    pub prefab: String,
    pub transform: SceneTransformV2,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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

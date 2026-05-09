use bevy::prelude::*;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

pub const GAME_SCENE_V2_SCHEMA_VERSION: u32 = 2;

/// Tonemapping algorithm applied to all cameras in a scene.
///
/// `TonyMcMapface` and `BlenderFilmic` are intentionally excluded — they require a
/// LUT texture lookup that reduces performance and breaks consistency across platforms.
/// HDR and bloom are not supported for the same reason: performant web builds are
/// the baseline, and the engine prioritises a consistent look on all platforms over
/// native-only visual upgrades.
#[derive(Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TonemappingOption {
    /// Cinematic, high-contrast filmic tonemapper. High performance (no LUT).
    /// Good default for most 3D scenes.
    #[default]
    AcesFitted,
    /// No tonemapping. Raw linear output; colours clip at 1.0.
    /// Useful for stylised looks or purely flat scenes.
    None,
    /// Smooth, muted tonemapper. Can appear "washed out" at high exposures.
    Reinhard,
    /// Like Reinhard but preserves colour hue better in high-contrast areas.
    ReinhardLuminance,
    /// Neutral, predictable transform with minimal artistic flavour.
    SomewhatBoringDisplayTransform,
}

impl TonemappingOption {
    pub fn to_bevy(self) -> bevy::core_pipeline::tonemapping::Tonemapping {
        use bevy::core_pipeline::tonemapping::Tonemapping;
        match self {
            Self::AcesFitted => Tonemapping::AcesFitted,
            Self::None => Tonemapping::None,
            Self::Reinhard => Tonemapping::Reinhard,
            Self::ReinhardLuminance => Tonemapping::ReinhardLuminance,
            Self::SomewhatBoringDisplayTransform => Tonemapping::SomewhatBoringDisplayTransform,
        }
    }
}

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
    pub spawn_points: BTreeMap<String, (f32, f32, f32)>,
    #[serde(default)]
    pub entities: Vec<SceneEntityDef>,
    #[serde(default)]
    pub ui: Vec<UiNodeDef>,
    /// When set, the UI elements are laid out in a centered panel box instead of
    /// using absolute positioning. `position` on each element is ignored in this mode.
    #[serde(default)]
    pub ui_panel: Option<UiPanelDef>,
    /// Tonemapping applied to all cameras spawned for this scene.
    /// Defaults to `AcesFitted` when omitted.
    /// `TonyMcMapface` and `BlenderFilmic` are not available — see `TonemappingOption`.
    #[serde(default)]
    pub tonemapping: TonemappingOption,
    /// Per-scene key bindings. Keys present here override `global_key_bindings` from the
    /// project config for this scene. Cleared and rebuilt from the project config each time
    /// a new scene loads, so a later scene cannot accidentally inherit bindings from an earlier one.
    /// Same key-name format as `global_key_bindings` (e.g. `"Escape"`, `"Space"`, `"KeyP"`).
    #[serde(default)]
    pub scene_key_bindings: HashMap<String, String>,
    /// World-space billboard text labels. Each label is placed at a 3D position and
    /// automatically rotates to face the active camera. Use for row headers, area names,
    /// or any annotation that should exist in the 3D world rather than the screen overlay.
    #[serde(default)]
    pub world_labels: Vec<WorldLabelDef>,
    /// Depth-based label scaling for all labels in this scene.
    /// When set, labels shrink as the camera moves away from them.
    /// Individual labels can opt out via `depth_scale: Some(false)`.
    #[serde(default)]
    pub label_depth_scale: Option<LabelDepthScaleDef>,
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
            let id = elem.id();
            if id.is_empty() {
                return Err("UI element has empty id".to_string());
            }
            if !ui_ids.insert(id) {
                return Err(format!("Duplicate UI element id: \"{}\"", id));
            }
        }
        let mut wl_ids = std::collections::HashSet::new();
        for label in &self.world_labels {
            if label.id.is_empty() {
                return Err("World label has empty id".to_string());
            }
            if !wl_ids.insert(label.id.as_str()) {
                return Err(format!("Duplicate world label id: \"{}\"", label.id));
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
    /// Resolution of the directional-light shadow map texture (width = height).
    /// Applies globally to all directional lights in the scene.
    /// Must be a power of two. Bevy default: 2048. Use 1024 or 512 for better
    /// performance on small scenes; use 4096 for crisp shadows on large terrain.
    #[serde(default)]
    pub shadow_map_size: Option<u32>,
    /// Resolution of each cube-face of point-light shadow maps.
    /// Applies globally to all shadow-casting point lights in the scene.
    /// Must be a power of two. Bevy default: 512. Use 256 for cheaper point shadows.
    #[serde(default)]
    pub point_shadow_map_size: Option<u32>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DirectionalLightDefV2 {
    /// RGB color of the light as linear sRGB 0.0–1.0. Default: white `(1.0, 1.0, 1.0)`.
    #[serde(default = "default_directional_color")]
    pub color: (f32, f32, f32),
    /// Illuminance in lux. Default: 10000.0 (bright overcast sky).
    #[serde(default = "default_directional_intensity")]
    pub intensity: f32,
    /// Euler angles in degrees (XYZ order) orienting the light direction. Default: `(45, 0, 0)` (sunlight from above).
    #[serde(default = "default_directional_rotation")]
    pub rotation_euler_deg: (f32, f32, f32),
    #[serde(default = "default_true")]
    pub shadows_enabled: bool,
    /// Maximum distance in world units at which shadow cascades are rendered.
    /// Omit to use Bevy's default (~1000 units native, ~100 units WebGL).
    /// Tune downward for sharper cascades on a smaller scene.
    #[serde(default)]
    pub shadow_distance: Option<f32>,
    /// Fraction of each cascade's range that overlaps with the next cascade,
    /// used to blend the transition zone so the seam is invisible.
    /// Range 0.0–1.0. Bevy's built-in default is 0.2; 0.5 eliminates most
    /// visible seams on large flat surfaces. Omit to use Bevy's default.
    #[serde(default)]
    pub cascade_overlap: Option<f32>,
    /// Number of shadow cascade levels. Bevy default: 4.
    /// Fewer cascades mean fewer depth passes per frame — 2 is usually enough for
    /// small or medium scenes. Only meaningful when `shadows_enabled: true`.
    #[serde(default)]
    pub num_cascades: Option<u32>,
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
fn default_directional_color() -> (f32, f32, f32) { (1.0, 1.0, 1.0) }
fn default_directional_intensity() -> f32 { 10000.0 }
fn default_directional_rotation() -> (f32, f32, f32) { (45.0, 0.0, 0.0) }

#[derive(Deserialize, Debug, Clone, Component)]
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
    /// UV tiling scale for terrain layer textures. Higher values tile textures more finely.
    /// Defaults to 10.0.
    #[serde(default = "default_terrain_uv_scale")]
    pub uv_scale: f32,
}

fn default_chunk_size() -> u32 { 64 }
fn default_terrain_uv_scale() -> f32 { 10.0 }

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct SceneEntityDef {
    pub id: String,
    pub prefab: String,
    pub transform: SceneTransformV2,
    /// Optional text annotation that follows this entity as it moves.
    /// The label floats `offset` world units above the entity's origin.
    #[serde(default)]
    pub label: Option<EntityLabelDef>,
}

/// A text annotation attached to a scene entity.
/// Rendered in Camera2d screen space, projected from the entity's world position
/// plus `offset` each frame — so it appears to float above the object.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct EntityLabelDef {
    pub text: String,
    /// Offset from the entity's world origin in world units. Default: 7 units up.
    #[serde(default = "default_label_offset")]
    pub offset: (f32, f32, f32),
    /// Font size in screen pixels. Default: 18.
    #[serde(default = "default_wl_font_size")]
    pub font_size: f32,
    /// Label colour as linear RGBA (0.0–1.0). Default: near-white.
    #[serde(default = "default_wl_color")]
    pub color: (f32, f32, f32, f32),
    /// Per-label depth-scale override.
    /// `Some(false)` pins this label at its authored size regardless of the scene setting.
    /// `Some(true)` forces scaling on even if the scene has no `label_depth_scale` block.
    /// `None` (default) inherits the scene's `label_depth_scale` setting.
    #[serde(default)]
    pub depth_scale: Option<bool>,
}

fn default_label_offset() -> (f32, f32, f32) { (0.0, 7.0, 0.0) }

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

/// A typed UI node definition. The enum variant determines the element type;
/// each variant only exposes fields that are relevant to that type.
#[derive(Deserialize, Debug, Clone)]
pub enum UiNodeDef {
    Button(ButtonDef),
    Label(LabelDef),
    Rect(RectDef),
}

impl UiNodeDef {
    pub fn id(&self) -> &str {
        match self {
            UiNodeDef::Button(d) => &d.id,
            UiNodeDef::Label(d) => &d.id,
            UiNodeDef::Rect(d) => &d.id,
        }
    }
    pub fn size(&self) -> (f32, f32) {
        match self {
            UiNodeDef::Button(d) => d.size,
            UiNodeDef::Label(d) => d.size,
            UiNodeDef::Rect(d) => d.size,
        }
    }
    pub fn position(&self) -> (f32, f32) {
        match self {
            UiNodeDef::Button(d) => d.position,
            UiNodeDef::Label(d) => d.position,
            UiNodeDef::Rect(d) => d.position,
        }
    }
    pub fn absolute(&self) -> bool {
        match self {
            UiNodeDef::Button(d) => d.absolute,
            UiNodeDef::Label(d) => d.absolute,
            UiNodeDef::Rect(d) => d.absolute,
        }
    }
    pub fn align(&self) -> UiTextAlign {
        match self {
            UiNodeDef::Button(d) => d.align,
            UiNodeDef::Label(d) => d.align,
            UiNodeDef::Rect(_) => UiTextAlign::Center,
        }
    }
}

/// An interactive button that emits a `UiEvent::ButtonPressed` trigger when clicked.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ButtonDef {
    pub id: String,
    pub text: String,
    /// Trigger string; `"ui."` prefix is stripped when firing (e.g. `"ui.dance"` → `"dance"`).
    #[serde(default)]
    pub action: String,
    /// Top-left corner in pixels. Ignored in panel mode unless `absolute: true`.
    #[serde(default)]
    pub position: (f32, f32),
    /// Width and height in pixels. Default: `(120.0, 32.0)`.
    #[serde(default = "default_ui_size")]
    pub size: (f32, f32),
    /// Background colour as linear RGBA (0.0–1.0). Default: dark grey.
    #[serde(default = "default_ui_dark_color")]
    pub color: (f32, f32, f32, f32),
    /// Horizontal text alignment. Default: `Center`.
    #[serde(default)]
    pub align: UiTextAlign,
    /// In panel mode: position this element absolutely relative to the panel's
    /// top-left corner using its `position` field instead of flowing in the column.
    #[serde(default)]
    pub absolute: bool,
}

/// Non-interactive text display. Can be data-bound to a `GameVariables` key.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct LabelDef {
    pub id: String,
    #[serde(default)]
    pub text: String,
    /// Top-left corner in pixels. Ignored in panel mode unless `absolute: true`.
    #[serde(default)]
    pub position: (f32, f32),
    /// Width and height in pixels. Default: `(120.0, 32.0)`.
    #[serde(default = "default_ui_size")]
    pub size: (f32, f32),
    /// Horizontal text alignment. Default: `Center`.
    #[serde(default)]
    pub align: UiTextAlign,
    /// Name of a `GameVariables` key. When set, the label text is replaced every
    /// frame with the variable's current value.
    #[serde(default)]
    pub bind: Option<String>,
    /// Template used with `bind`. `"{}"` is replaced by the variable value
    /// (e.g. `"Score: {}"`). Defaults to the raw value when omitted.
    #[serde(default)]
    pub format: Option<String>,
    /// In panel mode: position this element absolutely relative to the panel's
    /// top-left corner using its `position` field instead of flowing in the column.
    #[serde(default)]
    pub absolute: bool,
}

/// Non-interactive coloured rectangle. Used for decorative backgrounds, dividers, and map tiles.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct RectDef {
    pub id: String,
    /// Top-left corner in pixels. Ignored in panel mode unless `absolute: true`.
    #[serde(default)]
    pub position: (f32, f32),
    /// Width and height in pixels. Default: `(120.0, 32.0)`.
    #[serde(default = "default_ui_size")]
    pub size: (f32, f32),
    /// Fill colour as linear RGBA (0.0–1.0). Default: dark grey.
    #[serde(default = "default_ui_dark_color")]
    pub color: (f32, f32, f32, f32),
    /// In panel mode: position this element absolutely relative to the panel's
    /// top-left corner using its `position` field instead of flowing in the column.
    #[serde(default)]
    pub absolute: bool,
}

#[derive(Deserialize, Debug, Clone, Copy, Default, PartialEq)]
pub enum UiTextAlign {
    Left,
    #[default]
    Center,
    Right,
}

/// When present on a scene, UI elements are laid out in a centered panel box
/// instead of using absolute positioning.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct UiPanelDef {
    /// Background color of the panel box as RGBA (0.0–1.0). Default: near-black `(0.1, 0.1, 0.1, 0.95)`.
    #[serde(default = "default_panel_bg")]
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
    /// Optional fixed height of the panel in pixels. Auto-sized if omitted.
    /// Set this when the panel contains absolutely-positioned children (e.g. a map),
    /// so the panel has a known size to contain them.
    #[serde(default)]
    pub height: Option<f32>,
}

fn default_panel_bg() -> (f32, f32, f32, f32) { (0.1, 0.1, 0.1, 0.95) }
fn default_panel_padding() -> f32 { 20.0 }
fn default_panel_gap() -> f32 { 12.0 }
fn default_ui_size() -> (f32, f32) { (120.0, 32.0) }
fn default_ui_dark_color() -> (f32, f32, f32, f32) { (0.15, 0.15, 0.15, 1.0) }

/// A text annotation anchored to a 3-D world position.
/// The engine projects `translation` through the active Camera3d each frame
/// and repositions the label in Camera2d screen space, so it appears to float
/// above that world point at a fixed readable size regardless of camera distance.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct WorldLabelDef {
    pub id: String,
    pub text: String,
    /// 3-D world position the label tracks (typically above the object of interest).
    pub translation: (f32, f32, f32),
    /// Font size in screen pixels. Default: 18.
    #[serde(default = "default_wl_font_size")]
    pub font_size: f32,
    /// Label colour as linear RGBA (0.0–1.0). Default: near-white.
    #[serde(default = "default_wl_color")]
    pub color: (f32, f32, f32, f32),
    /// Per-label depth-scale override.
    /// `Some(false)` pins this label at its authored size regardless of the scene setting.
    /// `Some(true)` forces scaling on even if the scene has no `label_depth_scale` block.
    /// `None` (default) inherits the scene's `label_depth_scale` setting.
    #[serde(default)]
    pub depth_scale: Option<bool>,
}

/// Scene-level depth-scale configuration for all labels.
/// Labels shrink as camera distance increases; `font_size` is the size at `reference_distance`.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct LabelDepthScaleDef {
    /// Camera distance at which labels render at their authored `font_size` (1:1).
    /// Labels further away shrink proportionally; labels closer stay at 1:1.
    #[serde(default = "default_label_ref_distance")]
    pub reference_distance: f32,
    /// Minimum scale floor as a fraction of `font_size` (0.0–1.0).
    /// `Some(0.3)` means labels never shrink below 30% of their authored size.
    /// `None` means no floor — labels scale toward zero at extreme distances.
    #[serde(default)]
    pub min_scale: Option<f32>,
}

fn default_label_ref_distance() -> f32 { 50.0 }

fn default_wl_font_size() -> f32 { 18.0 }
fn default_wl_color() -> (f32, f32, f32, f32) { (0.95, 0.95, 0.95, 1.0) }

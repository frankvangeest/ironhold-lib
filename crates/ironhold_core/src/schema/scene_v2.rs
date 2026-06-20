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
    /// Maximum live particle count for this scene. `Ambient` effects are silently skipped
    /// when the cap is reached; `Npc` effects are halved; `Player` effects always fire.
    /// Applied to `ParticleBudget` on scene load. Defaults to 2000 when omitted.
    #[serde(default)]
    pub particle_budget: Option<u32>,
    /// Ground-ring decal shown under the currently selected target entity.
    /// When set, a flat unlit quad tracks the selected entity's XZ position each frame.
    /// Disappears on `ClearTarget` or when the targeted entity is hidden/despawned.
    /// Omit this field to disable the indicator for this scene (no ring, no error).
    #[serde(default)]
    pub target_indicator: Option<TargetIndicatorDef>,
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
    /// Override the initial `base` value for named stats from the prefab's `stat_templates`.
    /// Keys are stat names (e.g. `"health"`); unknown keys emit a `warn!` at load time.
    /// `min`/`max`/`regen`/`thresholds` are unchanged — only the starting value differs.
    #[serde(default)]
    pub stat_overrides: HashMap<String, f32>,
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
    StatBar(StatBarDef),
    StatSpread(StatSpreadDef),
    StatRadar(StatRadarDef),
    ActionBar(ActionBarDef),
    DialoguePanel(DialoguePanelDef),
}

impl UiNodeDef {
    pub fn id(&self) -> &str {
        match self {
            UiNodeDef::Button(d) => &d.id,
            UiNodeDef::Label(d) => &d.id,
            UiNodeDef::Rect(d) => &d.id,
            UiNodeDef::StatBar(d) => &d.id,
            UiNodeDef::StatSpread(d) => &d.id,
            UiNodeDef::StatRadar(d) => &d.id,
            UiNodeDef::ActionBar(d) => &d.id,
            UiNodeDef::DialoguePanel(d) => &d.id,
        }
    }
    pub fn size(&self) -> (f32, f32) {
        match self {
            UiNodeDef::Button(d) => d.size,
            UiNodeDef::Label(d) => d.size,
            UiNodeDef::Rect(d) => d.size,
            UiNodeDef::StatBar(d) => d.size,
            UiNodeDef::StatSpread(d) => {
                let n = d.stats.len() as f32;
                let h = n * d.row_height + (n - 1.0).max(0.0) * d.row_gap;
                (d.label_width + d.bar_width, h)
            }
            UiNodeDef::StatRadar(d) => d.size,
            UiNodeDef::ActionBar(d) => {
                let n = d.slots.len() as f32;
                let w = n * d.slot_size + (n - 1.0).max(0.0) * d.slot_gap + 8.0;
                (w, d.slot_size + 8.0)
            }
            UiNodeDef::DialoguePanel(d) => d.size,
        }
    }
    pub fn position(&self) -> (f32, f32) {
        match self {
            UiNodeDef::Button(d) => d.position,
            UiNodeDef::Label(d) => d.position,
            UiNodeDef::Rect(d) => d.position,
            UiNodeDef::StatBar(d) => d.position,
            UiNodeDef::StatSpread(d) => d.position,
            UiNodeDef::StatRadar(d) => d.position,
            UiNodeDef::ActionBar(d) => d.position,
            UiNodeDef::DialoguePanel(d) => d.position,
        }
    }
    pub fn absolute(&self) -> bool {
        match self {
            UiNodeDef::Button(d) => d.absolute,
            UiNodeDef::Label(d) => d.absolute,
            UiNodeDef::Rect(d) => d.absolute,
            UiNodeDef::StatBar(d) => d.absolute,
            UiNodeDef::StatSpread(d) => d.absolute,
            UiNodeDef::StatRadar(d) => d.absolute,
            UiNodeDef::ActionBar(_) => true,
            UiNodeDef::DialoguePanel(_) => true,
        }
    }
    pub fn align(&self) -> UiTextAlign {
        match self {
            UiNodeDef::Button(d) => d.align,
            UiNodeDef::Label(d) => d.align,
            UiNodeDef::Rect(_) => UiTextAlign::Center,
            UiNodeDef::StatBar(_) => UiTextAlign::Center,
            UiNodeDef::StatSpread(_) => UiTextAlign::Center,
            UiNodeDef::StatRadar(_) => UiTextAlign::Center,
            UiNodeDef::ActionBar(_) => UiTextAlign::Center,
            UiNodeDef::DialoguePanel(_) => UiTextAlign::Left,
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

/// Ground-ring decal configuration for the selected-target indicator.
/// The indicator is a flat, double-sided, unlit `StandardMaterial` quad (alpha blend)
/// that tracks the selected entity's XZ position each frame. Not parented to the target
/// so it doesn't inherit animation scale transforms.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TargetIndicatorDef {
    /// Decal texture catalog key (from `assets.ron` `decals:` section).
    pub texture: String,
    /// Ring radius in metres (the quad is scaled to `radius * 2` in X and Z). Default: 1.0.
    #[serde(default = "default_indicator_radius")]
    pub radius: f32,
    /// RGBA tint applied to the decal texture. Default: cyan-white `(0.3, 0.8, 1.0, 0.75)`.
    #[serde(default = "default_indicator_color")]
    pub color: (f32, f32, f32, f32),
    /// Y lift above ground to avoid z-fighting. Default: 0.05.
    #[serde(default = "default_indicator_offset_y")]
    pub offset_y: f32,
    /// Named colour palette for `indicator_category` lookups on prefabs.
    /// Key = category string (e.g. `"enemy"`, `"ally"`, `"loot"`); value = RGBA tint.
    /// A prefab whose category key is absent falls through to `color`.
    #[serde(default)]
    pub named_colors: std::collections::HashMap<String, (f32, f32, f32, f32)>,
}

fn default_indicator_radius() -> f32 { 1.0 }
fn default_indicator_color() -> (f32, f32, f32, f32) { (1.0, 0.15, 0.15, 0.85) }
fn default_indicator_offset_y() -> f32 { 0.05 }

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

// ─── Stat display ─────────────────────────────────────────────────────────────

/// Fill direction of a stat bar.
#[derive(Deserialize, Debug, Clone, Copy, Default, PartialEq)]
pub enum BarOrientation {
    /// Fill left-to-right.
    #[default]
    Horizontal,
    /// Fill bottom-to-top.
    Vertical,
}

/// Row / column layout for a `StatSpread` panel.
#[derive(Deserialize, Debug, Clone, Copy, Default, PartialEq)]
pub enum StatSpreadLayout {
    /// One labelled row per stat.
    #[default]
    Rows,
}

/// Optional fill-colour override when the stat crosses a threshold.
/// `above_percent` is in the range 0.0–1.0; the band with the highest value
/// that is still ≤ the current fill ratio is selected.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ColorBand {
    pub above_percent: f32,
    pub color: (f32, f32, f32, f32),
}

/// A horizontal (or vertical) bar that fills proportionally to `current / max`
/// for a named stat. Updated automatically each frame — no event wiring needed.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct StatBarDef {
    pub id: String,
    /// Key of the stat to display (matches a key in `stats.ron`).
    pub stat_key: String,
    #[serde(default)]
    pub orientation: BarOrientation,
    /// Top-left corner in pixels. Ignored in panel mode unless `absolute: true`.
    #[serde(default)]
    pub position: (f32, f32),
    /// Width and height in pixels. Default: `(200.0, 20.0)`.
    #[serde(default = "default_stat_bar_size")]
    pub size: (f32, f32),
    /// Colour of the filled portion. Default: red.
    #[serde(default = "default_bar_fill_color")]
    pub fill_color: (f32, f32, f32, f32),
    /// Colour of the unfilled portion behind the fill. Default: dark red.
    #[serde(default = "default_bar_bg_color")]
    pub background_color: (f32, f32, f32, f32),
    /// Show a `"current / max"` text overlay centred on the bar. Default: false.
    #[serde(default)]
    pub show_value: bool,
    /// Threshold-based colour bands. The highest `above_percent` ≤ the current
    /// fill ratio is chosen; when no band matches, `fill_color` is used.
    #[serde(default)]
    pub color_bands: Vec<ColorBand>,
    #[serde(default)]
    pub absolute: bool,
}

/// A panel listing multiple stats as labelled minibar rows.
/// Each row shows the stat name, a minibar fill, and optionally the numeric value.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct StatSpreadDef {
    pub id: String,
    /// Stat keys to display (each matches a key in `stats.ron`).
    pub stats: Vec<String>,
    #[serde(default)]
    pub layout: StatSpreadLayout,
    /// Top-left corner in pixels. Ignored in panel mode unless `absolute: true`.
    #[serde(default)]
    pub position: (f32, f32),
    /// Width of the stat-name label column in pixels. Default: 80.0.
    #[serde(default = "default_spread_label_width")]
    pub label_width: f32,
    /// Width of the minibar column in pixels. Default: 120.0.
    #[serde(default = "default_spread_bar_width")]
    pub bar_width: f32,
    /// Height of each row in pixels. Default: 22.0.
    #[serde(default = "default_spread_row_height")]
    pub row_height: f32,
    /// Vertical gap between rows in pixels. Default: 4.0.
    #[serde(default = "default_spread_row_gap")]
    pub row_gap: f32,
    /// Label text colour as linear RGBA. Default: near-white.
    #[serde(default = "default_wl_color")]
    pub label_color: (f32, f32, f32, f32),
    /// Fill colour for each minibar. Default: blue.
    #[serde(default = "default_spread_fill_color")]
    pub bar_fill_color: (f32, f32, f32, f32),
    /// Background colour behind each minibar fill. Default: dark blue.
    #[serde(default = "default_spread_bg_color")]
    pub bar_background_color: (f32, f32, f32, f32),
    /// Show `"current / max"` text after each minibar. Default: true.
    #[serde(default = "default_true")]
    pub show_values: bool,
    #[serde(default)]
    pub absolute: bool,
}

fn default_stat_bar_size() -> (f32, f32) { (200.0, 20.0) }
fn default_bar_fill_color() -> (f32, f32, f32, f32) { (0.85, 0.15, 0.15, 1.0) }
fn default_bar_bg_color() -> (f32, f32, f32, f32) { (0.25, 0.10, 0.10, 1.0) }
fn default_spread_label_width() -> f32 { 80.0 }
fn default_spread_bar_width() -> f32 { 120.0 }
fn default_spread_row_height() -> f32 { 22.0 }
fn default_spread_row_gap() -> f32 { 4.0 }
fn default_spread_fill_color() -> (f32, f32, f32, f32) { (0.3, 0.6, 1.0, 1.0) }
fn default_spread_bg_color() -> (f32, f32, f32, f32) { (0.1, 0.1, 0.25, 1.0) }

/// An N-sided (3–12) radar/spider chart that fills each axis proportionally
/// to a named stat's `current / max` ratio.  Updated automatically each frame
/// by `stat_radar_update_system` — no event wiring needed.
///
/// The grid and outer boundary are straight-edged polygons (no circles).
/// Labels for each axis are not rendered by the current implementation and are
/// planned as a follow-up (`stat_radar_labels` backlog item).
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct StatRadarDef {
    pub id: String,
    /// Stat keys to display, 3–12 entries (matches keys in `stats.ron`).
    pub stats: Vec<String>,
    /// Width and height of the bounding square in pixels. Default: `(240.0, 240.0)`.
    #[serde(default = "default_radar_size")]
    pub size: (f32, f32),
    /// Top-left corner in pixels. Ignored in panel mode unless `absolute: true`.
    #[serde(default)]
    pub position: (f32, f32),
    /// Number of concentric grid rings drawn inside the polygon. Default: 3.
    #[serde(default = "default_radar_grid_steps")]
    pub grid_steps: u32,
    /// Width of the polygon outline in pixels. Converted to UV fractions at spawn time
    /// using the smaller of `size.width` / `size.height`. Default: 2.0 px.
    #[serde(default = "default_radar_outline_width")]
    pub outline_width: f32,
    /// Fill colour as linear RGBA. Default: blue, 45 % opacity.
    #[serde(default = "default_radar_fill_color")]
    pub fill_color: (f32, f32, f32, f32),
    /// Polygon outline colour as linear RGBA. Default: bright blue, opaque.
    #[serde(default = "default_radar_outline_color")]
    pub outline_color: (f32, f32, f32, f32),
    /// Grid ring and spoke colour as linear RGBA. Default: grey, 45 % opacity.
    #[serde(default = "default_radar_grid_color")]
    pub grid_color: (f32, f32, f32, f32),
    /// Background colour inside the max circle as linear RGBA. Default: dark blue, 80 % opacity.
    #[serde(default = "default_radar_background_color")]
    pub background_color: (f32, f32, f32, f32),
    #[serde(default)]
    pub absolute: bool,
}

fn default_radar_size() -> (f32, f32) { (240.0, 240.0) }
fn default_radar_grid_steps() -> u32 { 3 }
fn default_radar_outline_width() -> f32 { 2.0 }
fn default_radar_fill_color() -> (f32, f32, f32, f32) { (0.35, 0.65, 1.0, 0.45) }
fn default_radar_outline_color() -> (f32, f32, f32, f32) { (0.55, 0.85, 1.0, 1.0) }
fn default_radar_grid_color() -> (f32, f32, f32, f32) { (0.40, 0.45, 0.55, 0.45) }
fn default_radar_background_color() -> (f32, f32, f32, f32) { (0.10, 0.12, 0.20, 0.80) }

// ─── Action bar ───────────────────────────────────────────────────────────────

/// A row of up to 9 skill slots bound to keys 1–9.
/// Pressing a slot key fires its `do_actions` through the existing pipeline,
/// checks an optional cooldown, and deducts an optional stat cost.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ActionBarDef {
    pub id: String,
    /// Top-left corner in pixels (always absolute).
    #[serde(default)]
    pub position: (f32, f32),
    /// Width and height of each slot square in pixels. Default: 64.0.
    #[serde(default = "default_slot_size")]
    pub slot_size: f32,
    /// Pixel gap between slots. Default: 4.0.
    #[serde(default = "default_slot_gap")]
    pub slot_gap: f32,
    /// Background colour of the bar container as linear RGBA. Default: near-black, 70 % alpha.
    #[serde(default = "default_bar_bg")]
    pub background_color: (f32, f32, f32, f32),
    pub slots: Vec<ActionSlotDef>,
    /// Texture catalog key for the icon atlas sheet shared by all slots in this bar.
    /// When set, slots with `icon_index` show the corresponding cell from this atlas.
    #[serde(default)]
    pub icon_sheet: Option<String>,
    /// Columns in the icon atlas grid. Default: 4.
    #[serde(default = "default_icon_cols")]
    pub icon_cols: u32,
    /// Rows in the icon atlas grid. Default: 4.
    #[serde(default = "default_icon_rows")]
    pub icon_rows: u32,
    /// Pixel size of each square cell in the atlas. Default: 64.
    #[serde(default = "default_icon_cell_size")]
    pub icon_cell_size: u32,
}

/// One slot in an `ActionBar`.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ActionSlotDef {
    /// Key that activates this slot: `"1"` through `"9"`.
    pub key: String,
    /// Per-slot texture catalog key override. When non-empty, overrides the bar's `icon_sheet`
    /// for this slot. Leave empty to use the bar-level `icon_sheet`.
    #[serde(default)]
    pub icon: String,
    /// Zero-based index into the icon atlas (row-major). `icon_sheet` on the bar must be set.
    /// Row 0 = top row; index `col + row * icon_cols`. Default: 0.
    #[serde(default)]
    pub icon_index: u32,
    /// Linear RGBA tint multiplied onto the icon image. Omit for no tint; `(1,1,1,1)` is equivalent.
    #[serde(default)]
    pub icon_color: Option<(f32, f32, f32, f32)>,
    /// Actions fired through the pipeline when the slot activates.
    pub do_actions: Vec<crate::schema::actions::Action>,
    /// Seconds before this slot can be used again. Omit for no cooldown.
    #[serde(default)]
    pub cooldown_secs: Option<f32>,
    /// Stat cost deducted at activation time. Activation is blocked if the stat is
    /// below `amount`; emits `action_bar.insufficient_resource:{key}` instead.
    #[serde(default)]
    pub cost: Option<SlotCost>,
    /// Optional tooltip label shown on hover (future use).
    #[serde(default)]
    pub label: Option<String>,
}

/// Stat cost for an `ActionSlotDef`.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct SlotCost {
    /// Key of the stat to check and deduct from (matches a key in `stats.ron`).
    pub stat: String,
    /// Amount to deduct. Activation is blocked if current value < amount.
    pub amount: f32,
}

fn default_slot_size() -> f32 { 64.0 }
fn default_slot_gap()  -> f32 { 4.0 }
fn default_bar_bg() -> (f32, f32, f32, f32) { (0.0, 0.0, 0.0, 0.70) }
fn default_icon_cols() -> u32 { 4 }
fn default_icon_rows() -> u32 { 4 }
fn default_icon_cell_size() -> u32 { 64 }

// ─── Dialogue panel ───────────────────────────────────────────────────────────

/// A data-driven dialogue panel that displays NPC conversation text and player choices.
/// Spawned as a UI element; visibility is managed at runtime by `dialogue_tick_system`.
/// Always uses absolute positioning. Start hidden by default (`initially_hidden: true`).
///
/// ```ron
/// DialoguePanel((
///     id: "npc_panel",
///     position: (16.0, 430.0),
///     size: (1200.0, 200.0),
/// ))
/// ```
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DialoguePanelDef {
    pub id: String,
    /// Top-left corner in pixels (always absolute).
    pub position: (f32, f32),
    /// Width and height of the panel in pixels.
    pub size: (f32, f32),
    /// Panel background colour as linear RGBA. Default: near-black, 92 % alpha.
    #[serde(default = "default_dialogue_bg")]
    pub background_color: (f32, f32, f32, f32),
    /// Font size for the speaker name line. Default: 18.
    #[serde(default = "default_speaker_font_size")]
    pub speaker_font_size: f32,
    /// Font size for the body text. Default: 15.
    #[serde(default = "default_body_font_size")]
    pub body_font_size: f32,
    /// Font size for dynamically spawned choice buttons. Default: 13.
    #[serde(default = "default_choice_font_size")]
    pub choice_font_size: f32,
    /// When `true` (default) the panel starts invisible and is shown by the dialogue system.
    #[serde(default = "default_true_bool")]
    pub initially_hidden: bool,
}

fn default_dialogue_bg() -> (f32, f32, f32, f32) { (0.05, 0.05, 0.08, 0.92) }
fn default_speaker_font_size() -> f32 { 18.0 }
fn default_body_font_size() -> f32 { 15.0 }
fn default_choice_font_size() -> f32 { 13.0 }
fn default_true_bool() -> bool { true }

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use super::material::MaterialDef;

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct AssetCatalog {
    #[serde(default)]
    pub models: HashMap<String, ModelCatalogEntry>,
    #[serde(default)]
    pub textures: HashMap<String, String>,
    #[serde(default)]
    pub audio: HashMap<String, String>,
    #[serde(default)]
    pub materials: HashMap<String, MaterialDef>,
}

impl Default for AssetCatalog {
    fn default() -> Self {
        Self {
            models: HashMap::new(),
            textures: HashMap::new(),
            audio: HashMap::new(),
            materials: HashMap::new(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelCatalogEntry {
    pub path: String,
}

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct PrefabCatalog {
    #[serde(default)]
    pub prefabs: HashMap<String, PrefabDef>,
}

impl Default for PrefabCatalog {
    fn default() -> Self {
        Self {
            prefabs: HashMap::new(),
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct PrefabDef {
    pub kind: String,   // "actor", "prop", or "primitive"
    pub model: String,  // key into AssetCatalog.models; repurposed as shape name for "primitive" kind
    #[serde(default)]
    pub animation_policy: Option<String>,
    /// Optional material key from AssetCatalog.materials to override the model's embedded material.
    #[serde(default)]
    pub material: Option<String>,
    #[serde(default)]
    pub components: PrefabComponents,
    /// Shape parameters for `kind: "primitive"`. All fields are optional; hardcoded defaults apply
    /// when omitted so minimal RON is still valid.
    #[serde(default)]
    pub primitive: Option<PrimitiveParams>,
}

/// Runtime-relevant prefab component data.
/// Additional design-time fields (health, ai, etc.) are silently ignored.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct PrefabComponents {
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Dimension and appearance overrides for `kind: "primitive"` prefabs.
///
/// Field semantics by shape:
/// - `size`       → Cuboid (x, y, z)
/// - `radius`     → Sphere radius | Cylinder/Capsule/Cone radius | Torus outer radius | ConicalFrustum bottom radius
/// - `radius_top` → ConicalFrustum top radius | Torus inner radius
/// - `height`     → Cylinder height | Capsule half_length | Cone height | ConicalFrustum height
/// - `color`      → base color as linear sRGB (r, g, b) in the 0.0–1.0 range
/// - `roughness`  → perceptual roughness (0 = mirror, 1 = fully rough; default 0.5)
/// - `metallic`   → metallic factor (0 = dielectric, 1 = full metal; default 0.0)
#[derive(Deserialize, Debug, Clone, Default)]
pub struct PrimitiveParams {
    #[serde(default)]
    pub size: Option<(f32, f32, f32)>,
    #[serde(default)]
    pub radius: Option<f32>,
    #[serde(default)]
    pub radius_top: Option<f32>,
    #[serde(default)]
    pub height: Option<f32>,
    #[serde(default)]
    pub color: Option<(f32, f32, f32)>,
    #[serde(default)]
    pub roughness: Option<f32>,
    #[serde(default)]
    pub metallic: Option<f32>,
}

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct AssetCatalog {
    #[serde(default)]
    pub models: HashMap<String, ModelCatalogEntry>,
    #[serde(default)]
    pub textures: HashMap<String, String>,
    #[serde(default)]
    pub audio: HashMap<String, String>,
    // #[serde(default)]
    // pub materials: HashMap<String, MaterialDef>,
}

impl Default for AssetCatalog {
    fn default() -> Self {
        Self {
            models: HashMap::new(),
            textures: HashMap::new(),
            audio: HashMap::new(),
            // materials: HashMap::new(),
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

#[derive(Deserialize, Debug, Clone)]
pub struct PrefabDef {
    pub kind: String,   // "actor" or "prop"
    pub model: String,  // key into AssetCatalog.models
    #[serde(default)]
    pub animation_policy: Option<String>,
    #[serde(default)]
    pub components: PrefabComponents,
}

/// Runtime-relevant prefab component data.
/// Additional design-time fields (health, ai, etc.) are silently ignored.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct PrefabComponents {
    #[serde(default)]
    pub tags: Vec<String>,
}

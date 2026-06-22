use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

pub const ITEM_CATALOG_SCHEMA_VERSION: u32 = 1;

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct ItemCatalog {
    pub schema_version: u32,
    #[serde(default)]
    pub items: HashMap<String, ItemDef>,
}

impl ItemCatalog {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != ITEM_CATALOG_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported ItemCatalog schema_version {} (expected {})",
                self.schema_version, ITEM_CATALOG_SCHEMA_VERSION
            ));
        }
        for (key, item) in &self.items {
            if item.display_name.is_empty() {
                return Err(format!("ItemCatalog item \"{}\" has empty display_name", key));
            }
            if item.max_stack == 0 {
                return Err(format!("ItemCatalog item \"{}\" max_stack must be at least 1", key));
            }
        }
        Ok(())
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ItemDef {
    pub display_name: String,
    /// Catalog texture key for the icon atlas sheet this item's icon comes from.
    /// When set, overrides the `InventoryPanelDef.icon_sheet` default for this item.
    /// The sheet must use the same grid dimensions (`icon_cols/rows/cell_size`) as the panel.
    /// Omit for items whose icon is on the panel's default sheet.
    #[serde(default)]
    pub icon_sheet: Option<String>,
    /// Zero-based index into the icon atlas (row-major): `col + row * icon_cols`. Default: 0.
    #[serde(default)]
    pub icon_index: u32,
    /// Linear RGBA color multiplied onto the icon. Omit for no tint (defaults to white).
    /// Use this to re-color a greyscale or lightly-colored icon (e.g. `(1.0, 0.3, 0.3, 1.0)` for red).
    #[serde(default)]
    pub icon_color: Option<(f32, f32, f32, f32)>,
    /// When true, multiple units stack in one inventory slot. Default: true.
    #[serde(default = "default_stackable")]
    pub stackable: bool,
    /// Max units per stack (ignored when stackable: false, treated as 1). Default: 99.
    #[serde(default = "default_max_stack")]
    pub max_stack: u32,
    /// Item weight in arbitrary units. Default: 1.0.
    #[serde(default = "default_weight")]
    pub weight: f32,
    /// Designer-defined tags for filtering and condition matching.
    #[serde(default)]
    pub tags: Vec<String>,
    /// When set, looting this item adds its count to the named global stat instead of
    /// placing it in the player's inventory slots. Use for currency items (e.g. "gold").
    #[serde(default)]
    pub currency_stat: Option<String>,
}

fn default_stackable() -> bool { true }
fn default_max_stack() -> u32 { 99 }
fn default_weight() -> f32 { 1.0 }

/// A stack of one item type occupying one inventory slot.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemStack {
    pub item_key: String,
    pub count: u32,
}

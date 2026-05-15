use bevy::prelude::*;
use crate::schema::stats::{LoadedStats, StatMap};
use crate::schema::scene_v2::BarOrientation;
use crate::runtime::scene_manager::SpawnId;

/// Resolves a stat by key from either `LoadedStats` (global) or `StatMap` (per-entity).
///
/// Keys without a dot (e.g. `"player_health"`) look up the global `LoadedStats` resource.
/// Keys with a dot (e.g. `"dummy_01.health"`) split on the first dot, find the entity whose
/// `SpawnId` matches, and look up the stat name in its `StatMap` component.
///
/// Returns `(effective, min, max)` or `None` when the stat or entity is not found.
pub fn resolve_stat(
    key: &str,
    loaded_stats: &LoadedStats,
    stat_map_query: &Query<(&SpawnId, &StatMap)>,
) -> Option<(f32, f32, f32)> {
    if let Some(dot_pos) = key.find('.') {
        let entity_id = &key[..dot_pos];
        let stat_name = &key[dot_pos + 1..];
        stat_map_query
            .iter()
            .find(|(id, _)| id.0 == entity_id)
            .and_then(|(_, map)| map.0.get(stat_name))
            .map(|s| (s.effective, s.def.min, s.def.max))
    } else {
        loaded_stats.0.get(key).map(|s| (s.effective, s.def.min, s.def.max))
    }
}

/// Marker on the fill-rect child inside a `StatBar` or a `StatSpread` row's minibar.
/// `stat_bar_update_system` reads the stat via `resolve_stat` and updates `Node::width`
/// (horizontal) or `Node::height` (vertical) each frame.
#[derive(Component, Clone)]
pub struct StatBarFill {
    pub stat_key: String,
    pub orientation: BarOrientation,
    /// Base fill colour (used when no colour band matches).
    pub fill_color: (f32, f32, f32, f32),
    /// Threshold-based colour overrides. Each entry is `(above_percent, color)`.
    /// The highest `above_percent` ≤ current fill ratio is selected.
    pub color_bands: Vec<(f32, (f32, f32, f32, f32))>,
}

/// Marker on the optional `"current / max"` text entity inside a `StatBar` or spread row.
#[derive(Component, Clone)]
pub struct StatValueText {
    pub stat_key: String,
}

/// Marker on a floating world-space `Text2d` entity that tracks a live stat.
/// Spawned at scene load from `PrefabDef.stat_label`; `{self}` is already resolved.
/// `stat_label_update_system` updates the text each frame via `resolve_stat`.
#[derive(Component, Clone)]
pub struct StatLabelMarker {
    pub stat_key: String,
    pub show_max: bool,
}

/// Updates the fill width/height and colour of every `StatBarFill` entity each frame.
/// Guards all writes so Bevy's change-detection only triggers when values actually change.
pub fn stat_bar_update_system(
    loaded_stats: Res<LoadedStats>,
    stat_map_query: Query<(&SpawnId, &StatMap)>,
    mut fill_query: Query<(&StatBarFill, &mut Node, &mut BackgroundColor)>,
) {
    for (fill, mut node, mut bg) in fill_query.iter_mut() {
        let ratio = if let Some((effective, min, max)) =
            resolve_stat(&fill.stat_key, &loaded_stats, &stat_map_query)
        {
            let range = max - min;
            if range <= 0.0 {
                1.0
            } else {
                ((effective - min) / range).clamp(0.0, 1.0)
            }
        } else {
            if cfg!(debug_assertions) {
                warn!("StatBar: stat_key {:?} not found — bar renders empty", fill.stat_key);
            }
            0.0
        };

        let new_pct = Val::Percent(ratio * 100.0);
        match fill.orientation {
            BarOrientation::Horizontal => {
                if node.width != new_pct {
                    node.width = new_pct;
                }
            }
            BarOrientation::Vertical => {
                if node.height != new_pct {
                    node.height = new_pct;
                }
            }
        }

        if !fill.color_bands.is_empty() {
            let chosen = fill
                .color_bands
                .iter()
                .filter(|(threshold, _)| ratio >= *threshold)
                .max_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(_, c)| *c)
                .unwrap_or(fill.fill_color);
            let (r, g, b, a) = chosen;
            let new_color = Color::srgba(r, g, b, a);
            if bg.0 != new_color {
                bg.0 = new_color;
            }
        }
    }
}

/// Updates the `"current / max"` text of every `StatValueText` entity each frame.
pub fn stat_bar_value_text_system(
    loaded_stats: Res<LoadedStats>,
    stat_map_query: Query<(&SpawnId, &StatMap)>,
    mut text_query: Query<(&StatValueText, &mut Text)>,
) {
    for (label, mut text) in text_query.iter_mut() {
        let new_text = if let Some((effective, _, max)) =
            resolve_stat(&label.stat_key, &loaded_stats, &stat_map_query)
        {
            format!("{:.0} / {:.0}", effective, max)
        } else {
            "? / ?".to_string()
        };
        if text.0 != new_text {
            *text = Text::new(new_text);
        }
    }
}

/// Marker on the fill entity of a world-space stat bar spawned from `PrefabDef.world_stat_bar`.
/// `world_stat_bar_update_system` reads the stat each frame and updates the text content
/// and colour. The background-track entity is static and carries no marker.
#[derive(Component, Clone)]
pub struct WorldStatBarFillMarker {
    pub stat_key: String,
    /// Total cell count (from `WorldStatBarDef.cells`); determines max `=` characters.
    pub cells: u8,
    /// Base fill colour used when no `color_bands` entry matches (or bands is empty).
    pub fill_color: (f32, f32, f32, f32),
    /// Threshold-based colour overrides. Each entry is `(above_ratio, color)`.
    /// The highest `above_ratio` ≤ current fill ratio is selected.
    /// When empty, the built-in green/yellow/red adaptive logic applies.
    pub color_bands: Vec<(f32, (f32, f32, f32, f32))>,
}

/// Updates the fill text and colour of every `WorldStatBarFillMarker` entity.
/// When `color_bands` is set, the highest band whose threshold ≤ fill ratio is used.
/// When empty, falls back to adaptive green ≥ 60 %, yellow 30–59 %, red < 30 %.
/// Guards writes for change-detection efficiency.
pub fn world_stat_bar_update_system(
    loaded_stats: Res<LoadedStats>,
    stat_map_query: Query<(&SpawnId, &StatMap)>,
    mut bar_query: Query<(&WorldStatBarFillMarker, &mut Text2d, &mut TextColor)>,
) {
    for (marker, mut text, mut color) in bar_query.iter_mut() {
        let (ratio, (cr, cg, cb, ca)) = if let Some((effective, min, max)) =
            resolve_stat(&marker.stat_key, &loaded_stats, &stat_map_query)
        {
            let range = max - min;
            let r = if range > 0.0 { ((effective - min) / range).clamp(0.0, 1.0) } else { 1.0 };
            let c = if !marker.color_bands.is_empty() {
                marker.color_bands
                    .iter()
                    .filter(|(threshold, _)| r >= *threshold)
                    .max_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(_, col)| *col)
                    .unwrap_or(marker.fill_color)
            } else {
                let (_, _, _, a) = marker.fill_color;
                if r >= 0.6 {
                    marker.fill_color
                } else if r >= 0.3 {
                    (0.95, 0.80, 0.10, a)
                } else {
                    (0.90, 0.15, 0.10, a)
                }
            };
            (r, c)
        } else {
            (0.0, marker.fill_color)
        };

        let cells = (marker.cells as usize).max(1);
        let filled = (ratio * cells as f32).round() as usize;
        let new_text = format!("{:<width$}", "=".repeat(filled), width = cells);
        if text.0 != new_text {
            text.0 = new_text;
        }
        let new_color = Color::srgba(cr, cg, cb, ca);
        if color.0 != new_color {
            color.0 = new_color;
        }
    }
}

/// Updates floating stat labels spawned from `PrefabDef.stat_label` each frame.
/// Uses `resolve_stat` so both global (`"player_health"`) and entity-local
/// (`"dummy_01.health"`) keys work transparently.
pub fn stat_label_update_system(
    loaded_stats: Res<LoadedStats>,
    stat_map_query: Query<(&SpawnId, &StatMap)>,
    mut label_query: Query<(&StatLabelMarker, &mut Text2d)>,
) {
    for (marker, mut text) in label_query.iter_mut() {
        let new_text = if let Some((effective, _, max)) =
            resolve_stat(&marker.stat_key, &loaded_stats, &stat_map_query)
        {
            if marker.show_max {
                format!("{:.0} / {:.0}", effective, max)
            } else {
                format!("{:.0}", effective)
            }
        } else {
            String::new()
        };
        if text.0 != new_text {
            text.0 = new_text;
        }
    }
}

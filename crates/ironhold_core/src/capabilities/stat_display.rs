use bevy::prelude::*;
use crate::schema::stats::LoadedStats;
use crate::schema::scene_v2::BarOrientation;

/// Marker on the fill-rect child inside a `StatBar` or a `StatSpread` row's minibar.
/// `stat_bar_update_system` reads `LoadedStats[stat_key]` and updates `Node::width`
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

/// Updates the fill width/height and colour of every `StatBarFill` entity each frame.
/// Guards all writes so Bevy's change-detection only triggers when values actually change.
pub fn stat_bar_update_system(
    loaded_stats: Res<LoadedStats>,
    mut fill_query: Query<(&StatBarFill, &mut Node, &mut BackgroundColor)>,
) {
    for (fill, mut node, mut bg) in fill_query.iter_mut() {
        let ratio = if let Some(stat) = loaded_stats.0.get(&fill.stat_key) {
            let range = stat.def.max - stat.def.min;
            if range <= 0.0 {
                1.0
            } else {
                ((stat.current - stat.def.min) / range).clamp(0.0, 1.0)
            }
        } else {
            if cfg!(debug_assertions) {
                warn!("StatBar: stat_key {:?} not found in LoadedStats — bar renders empty", fill.stat_key);
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
    mut text_query: Query<(&StatValueText, &mut Text)>,
) {
    for (label, mut text) in text_query.iter_mut() {
        let new_text = if let Some(stat) = loaded_stats.0.get(&label.stat_key) {
            format!("{:.0} / {:.0}", stat.current, stat.def.max)
        } else {
            "? / ?".to_string()
        };
        if text.0 != new_text {
            *text = Text::new(new_text);
        }
    }
}

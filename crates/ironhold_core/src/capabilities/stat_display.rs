use bevy::prelude::*;
use crate::schema::stats::{LoadedStats, StatMap};
use crate::schema::scene_v2::BarOrientation;
use crate::schema::catalog::{StatLabelDef, WorldStatBarDef, WorldStatBarStyle, AssetCatalog};
use crate::runtime::scene_manager::{SpawnId, WorldLabel, WorldLabelRank, LevelEntity};
use crate::capabilities::camera::MAX_SPLIT_PLAYERS;

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

/// Marker on the fill mesh of a `WorldStatBarStyle::Pixel` bar.
/// The entity is a child of an invisible anchor that carries `WorldLabel`.
/// `world_pixel_bar_update_system` drives `Transform.scale.x` (fill width) and the
/// `ColorMaterial` color each frame.
#[derive(Component, Clone)]
pub struct WorldPixelBarFillMarker {
    pub stat_key: String,
    /// Full bar width in screen pixels — fixed at spawn, used to compute fill width.
    pub full_width: f32,
    pub fill_color: (f32, f32, f32, f32),
    pub color_bands: Vec<(f32, (f32, f32, f32, f32))>,
}

/// Updates the fill mesh scale and color of every `WorldPixelBarFillMarker` entity.
/// `Transform.scale.x = ratio * full_width` (fill width).
/// `Transform.translation.x` is kept left-aligned within the bar.
/// Writes are guarded for change-detection efficiency.
pub fn world_pixel_bar_update_system(
    loaded_stats: Res<LoadedStats>,
    stat_map_query: Query<(&SpawnId, &StatMap)>,
    mut fill_query: Query<(&WorldPixelBarFillMarker, &mut Transform, &MeshMaterial2d<ColorMaterial>)>,
    color_materials: Option<ResMut<Assets<ColorMaterial>>>,
) {
    let Some(mut color_materials) = color_materials else { return };
    for (marker, mut transform, mat_handle) in fill_query.iter_mut() {
        let ratio = if let Some((effective, min, max)) =
            resolve_stat(&marker.stat_key, &loaded_stats, &stat_map_query)
        {
            let range = max - min;
            if range <= 0.0 { 1.0 } else { ((effective - min) / range).clamp(0.0, 1.0) }
        } else {
            if cfg!(debug_assertions) {
                warn!("WorldPixelBar: stat_key {:?} not found — bar renders empty", marker.stat_key);
            }
            0.0
        };

        let new_width = ratio * marker.full_width;
        if (transform.scale.x - new_width).abs() > 0.5 {
            transform.scale.x = new_width;
            transform.translation.x = -marker.full_width / 2.0 + new_width / 2.0;
        }

        let chosen = if !marker.color_bands.is_empty() {
            marker.color_bands.iter()
                .filter(|(t, _)| ratio >= *t)
                .max_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(_, c)| *c)
                .unwrap_or(marker.fill_color)
        } else {
            marker.fill_color
        };
        let (r, g, b, a) = chosen;
        let new_color = Color::srgba(r, g, b, a);
        if let Some(mat) = color_materials.get_mut(&mat_handle.0) {
            if mat.color != new_color {
                mat.color = new_color;
            }
        }
    }
}

/// On the anchor entity of a `WorldStatBarStyle::Icon` bar. `world_icon_bar_update_system`
/// resolves the stat ONCE per anchor, then walks its children (`Sprite` entities, spawned in
/// cell order 0..cells) to set each one's atlas index — not once per cell, since `resolve_stat`'s
/// dotted-key lookup is O(entities-with-`StatMap`) and paying that cost per cell would multiply
/// it up to 20x per bar per frame for no reason.
#[derive(Component, Clone)]
pub struct WorldIconBar {
    pub stat_key: String,
    pub cells: u8,
    pub filled_index: u32,
    pub empty_index: u32,
}

/// Updates every `WorldIconBar` anchor's child `Sprite` atlas indices each frame.
/// Fill count uses `ceil`, not `round`: `filled = 0` only at exactly `ratio == 0.0`, otherwise
/// `filled = max(1, ceil(ratio * cells))` — so e.g. 1% health always shows ≥1 filled cell (never
/// reads as dead while alive), and 95% shows full on a 5-cell bar (expected/idiomatic for this
/// style, not a bug — see `planning/features/world_icon_stat_bar.md`). Writes are guarded for
/// change-detection efficiency.
pub fn world_icon_bar_update_system(
    loaded_stats: Res<LoadedStats>,
    stat_map_query: Query<(&SpawnId, &StatMap)>,
    bar_query: Query<(&WorldIconBar, &Children)>,
    mut sprite_query: Query<&mut Sprite>,
) {
    for (bar, children) in bar_query.iter() {
        let ratio = if let Some((effective, min, max)) =
            resolve_stat(&bar.stat_key, &loaded_stats, &stat_map_query)
        {
            let range = max - min;
            if range <= 0.0 { 1.0 } else { ((effective - min) / range).clamp(0.0, 1.0) }
        } else {
            if cfg!(debug_assertions) {
                warn!("WorldIconBar: stat_key {:?} not found — bar renders empty", bar.stat_key);
            }
            0.0
        };

        let filled_count: u8 = if ratio <= 0.0 {
            0
        } else {
            (ratio * bar.cells as f32).ceil().max(1.0) as u8
        };

        for (i, child) in children.iter().enumerate() {
            let Ok(mut sprite) = sprite_query.get_mut(child) else { continue };
            let want_index = if (i as u8) < filled_count { bar.filled_index } else { bar.empty_index } as usize;
            // Read via `.as_ref()` first so an unchanged cell never triggers `Mut<Sprite>`'s
            // `DerefMut` (which would mark the Sprite changed regardless of whether the inner
            // field write actually happens) — `.as_mut()` is only reached on the branch that
            // genuinely writes.
            let needs_update = match sprite.texture_atlas.as_ref() {
                Some(a) => a.index != want_index,
                None => true,
            };
            if needs_update {
                if let Some(atlas) = sprite.texture_atlas.as_mut() {
                    atlas.index = want_index;
                }
            }
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

/// Shared spawn-time context for `spawn_stat_label_widget`/`spawn_world_stat_bar_widget`.
/// Bundles the handful of things that differ between the scene-load and dynamic-spawn call
/// sites (both resolve their own `depth_scale`/`is_split_screen` beforehand) so the widget
/// entity construction itself — previously duplicated across `scene_loader.rs`'s two spawn
/// loops and `drain_dynamic_stat_ui_system` — lives in exactly one place.
pub struct StatWidgetSpawnCtx<'a> {
    /// Only touched by `Pixel`-style bars.
    pub meshes: &'a mut Assets<Mesh>,
    /// Only touched by `Pixel`-style bars; `None` panics if a `Pixel` bar is actually spawned
    /// (mirrors the pre-extraction `.expect(...)` — every real caller has this available).
    pub color_materials: Option<&'a mut Assets<ColorMaterial>>,
    /// Pre-resolved via `resolve_label_depth_scale` at the call site (each call site reads a
    /// different source: the scene's `label_depth_scale` block vs. the `LoadedLabelDepthScale`
    /// resource) — this helper does not know or care which.
    pub depth_scale: Option<(f32, f32)>,
    /// Whether the loading/active scene is split-screen — gates `WorldLabelRank` sibling
    /// duplication for `stat_label`/Ascii/Icon bars (see `WorldLabelRank`'s doc comment).
    pub is_split_screen: bool,
    /// Only touched by `Icon`-style bars; `None` panics if an `Icon` bar is actually spawned.
    pub atlas_layouts: Option<&'a mut Assets<TextureAtlasLayout>>,
    /// Only touched by `Icon`-style bars; `None` panics if an `Icon` bar is actually spawned.
    pub asset_server: Option<&'a AssetServer>,
    /// Only touched by `Icon`-style bars; `None` panics if an `Icon` bar is actually spawned.
    pub asset_catalog: Option<&'a AssetCatalog>,
}

/// Spawns the floating `Text2d` widget for a `stat_label` (`StatLabelDef`), tracking `tracked`.
/// Duplicates one sibling per split-screen rank when `ctx.is_split_screen` (see `WorldLabelRank`).
pub fn spawn_stat_label_widget(
    commands: &mut Commands,
    tracked: Entity,
    stat_key: &str,
    def: &StatLabelDef,
    ctx: &StatWidgetSpawnCtx,
) {
    let (r, g, b, a) = def.color;
    let ranks = if ctx.is_split_screen { MAX_SPLIT_PLAYERS } else { 1 };
    for rank in 0..ranks {
        let mut label_entity = commands.spawn((
            Name::new(format!("StatLabel: {} (rank {})", stat_key, rank)),
            Text2d::new(String::new()),
            TextFont { font_size: def.font_size, ..default() },
            TextColor(Color::srgba(r, g, b, a)),
            Transform::from_xyz(0.0, 0.0, 1.0),
            WorldLabel {
                world_pos: Vec3::ZERO,
                tracked_entity: Some(tracked),
                offset: Vec3::from(def.offset),
                base_font_size: def.font_size,
                depth_scale: ctx.depth_scale,
                screen_offset: Vec2::ZERO,
            },
            StatLabelMarker { stat_key: stat_key.to_string(), show_max: def.show_max },
            LevelEntity,
        ));
        if rank > 0 {
            label_entity.insert((WorldLabelRank(rank as u8), Visibility::Hidden));
        }
    }
}

/// Spawns the floating widget for a `world_stat_bar` (`WorldStatBarDef`), tracking `tracked`.
/// Dispatches on `def.style`: `Ascii` → two rank-duplicated `Text2d` entities (background +
/// fill); `Pixel` → an anchor with up to 3 `Mesh2d` children (border/background/fill); `Icon` →
/// an anchor with `cells` `Sprite` children (per-cell atlas index, filled/empty). All three
/// styles duplicate one full set per split-screen rank when `ctx.is_split_screen` (see
/// `WorldLabelRank`'s doc comment). For `Pixel`, the border/background mesh and material handles
/// are registered once and cloned across ranks (identical geometry per rank), while the fill is
/// created fresh per rank since each tracks and updates independently. For `Icon`, the texture
/// and `TextureAtlasLayout` are likewise registered once and cloned across every rank/cell —
/// every cell of every rank shares identical atlas geometry, only the per-cell atlas *index*
/// (set by `world_icon_bar_update_system`) ever changes.
pub fn spawn_world_stat_bar_widget(
    commands: &mut Commands,
    tracked: Entity,
    stat_key: &str,
    def: &WorldStatBarDef,
    ctx: &mut StatWidgetSpawnCtx,
) {
    let offset_v3 = Vec3::from(def.offset);
    let fill_color = def.fill_color;
    let (fr, fg, fb, fa) = fill_color;
    let (bgr, bgg, bgb, bga) = def.bg_color;
    let color_bands = def.color_bands.clone();
    let ranks = if ctx.is_split_screen { MAX_SPLIT_PLAYERS } else { 1 };

    match def.style {
        WorldStatBarStyle::Ascii { cells, font_size } => {
            let cells_clamped = cells.max(1) as usize;
            let bg_chars = " ".repeat(cells_clamped);

            for rank in 0..ranks {
                // Background track — static, never updated.
                let mut bg_entity = commands.spawn((
                    Name::new(format!("StatBarBg: {} (rank {})", stat_key, rank)),
                    Text2d::new(bg_chars.clone()),
                    TextFont { font_size, ..default() },
                    TextColor(Color::srgba(bgr, bgg, bgb, bga)),
                    Transform::from_xyz(0.0, 0.0, 1.0),
                    WorldLabel {
                        world_pos: Vec3::ZERO,
                        tracked_entity: Some(tracked),
                        offset: offset_v3,
                        base_font_size: font_size,
                        depth_scale: ctx.depth_scale,
                        screen_offset: Vec2::ZERO,
                    },
                    LevelEntity,
                ));
                if rank > 0 {
                    bg_entity.insert((WorldLabelRank(rank as u8), Visibility::Hidden));
                }

                // Fill entity — text and colour updated each frame by world_stat_bar_update_system.
                let mut fill_entity = commands.spawn((
                    Name::new(format!("StatBarFill: {} (rank {})", stat_key, rank)),
                    Text2d::new(String::new()),
                    TextFont { font_size, ..default() },
                    TextColor(Color::srgba(fr, fg, fb, fa)),
                    Transform::from_xyz(0.0, 0.0, 2.0),
                    WorldLabel {
                        world_pos: Vec3::ZERO,
                        tracked_entity: Some(tracked),
                        offset: offset_v3,
                        base_font_size: font_size,
                        depth_scale: ctx.depth_scale,
                        screen_offset: Vec2::ZERO,
                    },
                    WorldStatBarFillMarker {
                        stat_key: stat_key.to_string(), cells, fill_color,
                        color_bands: color_bands.clone(),
                    },
                    LevelEntity,
                ));
                if rank > 0 {
                    fill_entity.insert((WorldLabelRank(rank as u8), Visibility::Hidden));
                }
            }
        }
        WorldStatBarStyle::Pixel { size, border, border_color } => {
            let w = size.0.max(1.0);
            let h = size.1.max(1.0);
            let b = border.clamp(0.0, h / 2.0);
            let (bdr, bdg, bdb, bda) = border_color;
            let color_mats = ctx.color_materials.as_mut()
                .expect("ColorMaterial assets must be available to spawn pixel stat bars");

            // Border/background geometry and material are identical across every rank of the
            // same bar instance (only the fill differs, and even then all ranks track the same
            // entity's same stat) — registered once here and cloned into each rank below, rather
            // than re-registered per rank.
            let border_mesh = (b > 0.0).then(|| ctx.meshes.add(Rectangle::new(w + 2.0 * b, h + 2.0 * b)));
            let border_mat = (b > 0.0).then(|| color_mats.add(ColorMaterial::from(Color::srgba(bdr, bdg, bdb, bda))));
            let bg_mesh = ctx.meshes.add(Rectangle::new(w, h));
            let bg_mat = color_mats.add(ColorMaterial::from(Color::srgba(bgr, bgg, bgb, bga)));

            for rank in 0..ranks {
                // Invisible anchor — WorldLabel tracks the entity; children follow via hierarchy.
                // Depth scaling intentionally not applied to Pixel-style bars (pre-existing
                // documented exclusion — see docs/20_data_formats.md's Pixel bar depth-scaling note).
                let mut anchor_cmds = commands.spawn((
                    Name::new(format!("PixelBarAnchor: {} (rank {})", stat_key, rank)),
                    Transform::default(),
                    Visibility::default(),
                    WorldLabel {
                        world_pos: Vec3::ZERO,
                        tracked_entity: Some(tracked),
                        offset: offset_v3,
                        base_font_size: 1.0,
                        depth_scale: None,
                        screen_offset: Vec2::ZERO,
                    },
                    LevelEntity,
                ));
                if rank > 0 {
                    anchor_cmds.insert((WorldLabelRank(rank as u8), Visibility::Hidden));
                }
                let anchor = anchor_cmds.id();

                // Border quad (skip when border <= 0) — shared mesh/material, cloned per rank.
                if let (Some(border_mesh), Some(border_mat)) = (&border_mesh, &border_mat) {
                    let border_child = commands.spawn((
                        Name::new(format!("PixelBarBorder: {} (rank {})", stat_key, rank)),
                        Mesh2d(border_mesh.clone()),
                        MeshMaterial2d(border_mat.clone()),
                        Transform::from_xyz(0.0, 0.0, 1.0),
                        LevelEntity,
                    )).id();
                    commands.entity(anchor).add_child(border_child);
                }

                // Background quad — full bar size, static; shared mesh/material, cloned per rank.
                let bg_child = commands.spawn((
                    Name::new(format!("PixelBarBg: {} (rank {})", stat_key, rank)),
                    Mesh2d(bg_mesh.clone()),
                    MeshMaterial2d(bg_mat.clone()),
                    Transform::from_xyz(0.0, 0.0, 2.0),
                    LevelEntity,
                )).id();
                commands.entity(anchor).add_child(bg_child);

                // Fill quad — width=1 mesh scaled per frame; left-aligned via transform.
                // scale.x = ratio * w; translation.x = -w/2 + (ratio*w)/2. Created fresh per rank:
                // each rank's fill entity is updated independently by world_pixel_bar_update_system.
                let fill_child = commands.spawn((
                    Name::new(format!("PixelBarFill: {} (rank {})", stat_key, rank)),
                    Mesh2d(ctx.meshes.add(Rectangle::new(1.0, h))),
                    MeshMaterial2d(color_mats.add(ColorMaterial::from(Color::srgba(fr, fg, fb, fa)))),
                    Transform::from_xyz(-w / 2.0, 0.0, 3.0)
                        .with_scale(Vec3::new(0.0, 1.0, 1.0)),
                    WorldPixelBarFillMarker {
                        stat_key: stat_key.to_string(), full_width: w, fill_color,
                        color_bands: color_bands.clone(),
                    },
                    LevelEntity,
                )).id();
                commands.entity(anchor).add_child(fill_child);
            }
        }
        WorldStatBarStyle::Icon {
            ref icon_sheet,
            icon_cols, icon_rows, icon_cell_size,
            filled_index, empty_index, cells, spacing, size,
        } => {
            let cells_clamped = cells.max(1);
            let cell_w = size.0.max(1.0);
            let cell_h = size.1.max(1.0);
            let gap = spacing.max(0.0);
            let total_width = cells_clamped as f32 * cell_w + (cells_clamped as f32 - 1.0).max(0.0) * gap;

            let atlas_layouts = ctx.atlas_layouts.as_mut()
                .expect("Assets<TextureAtlasLayout> must be available to spawn icon stat bars");
            let asset_server = ctx.asset_server
                .expect("AssetServer must be available to spawn icon stat bars");
            let asset_catalog = ctx.asset_catalog
                .expect("AssetCatalog must be available to spawn icon stat bars");

            // Texture + atlas layout are identical across every rank and every cell of the same
            // bar instance — built once here and cloned per rank/cell below, not re-registered
            // per entity (same sharing principle as Pixel's border/background mesh+material).
            let texture: Handle<Image> = asset_catalog.textures.get(icon_sheet.as_str())
                .map(|path| asset_server.load(path.clone()))
                .unwrap_or_default();
            let layout: Handle<TextureAtlasLayout> = atlas_layouts.add(TextureAtlasLayout::from_grid(
                UVec2::splat(icon_cell_size), icon_cols, icon_rows, None, None,
            ));

            for rank in 0..ranks {
                // Invisible anchor — WorldLabel tracks the entity; WorldIconBar resolves the
                // stat ONCE per anchor (not once per cell — see WorldIconBar's doc comment);
                // children follow via hierarchy. Depth scaling intentionally not applied
                // (same pre-existing exclusion as Pixel bars).
                let mut anchor_cmds = commands.spawn((
                    Name::new(format!("IconBarAnchor: {} (rank {})", stat_key, rank)),
                    Transform::default(),
                    Visibility::default(),
                    WorldLabel {
                        world_pos: Vec3::ZERO,
                        tracked_entity: Some(tracked),
                        offset: offset_v3,
                        base_font_size: 1.0,
                        depth_scale: None,
                        screen_offset: Vec2::ZERO,
                    },
                    WorldIconBar {
                        stat_key: stat_key.to_string(),
                        cells: cells_clamped,
                        filled_index,
                        empty_index,
                    },
                    LevelEntity,
                ));
                if rank > 0 {
                    anchor_cmds.insert((WorldLabelRank(rank as u8), Visibility::Hidden));
                }
                let anchor = anchor_cmds.id();

                // Cells spawned in order 0..cells_clamped — world_icon_bar_update_system relies
                // on this spawn order (walks &Children) to map cell index to fill state without
                // a per-cell marker component. Initial atlas index is empty_index; the update
                // system corrects it on the first tick after resolving the stat.
                for cell in 0..cells_clamped {
                    let x = -total_width / 2.0 + cell_w / 2.0 + cell as f32 * (cell_w + gap);
                    let cell_child = commands.spawn((
                        Name::new(format!("IconBarCell: {} #{} (rank {})", stat_key, cell, rank)),
                        Sprite {
                            image: texture.clone(),
                            texture_atlas: Some(TextureAtlas {
                                layout: layout.clone(),
                                index: empty_index as usize,
                            }),
                            custom_size: Some(Vec2::new(cell_w, cell_h)),
                            ..default()
                        },
                        Transform::from_xyz(x, 0.0, 1.0),
                        LevelEntity,
                    )).id();
                    commands.entity(anchor).add_child(cell_child);
                }
            }
        }
    }
}

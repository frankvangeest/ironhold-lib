use bevy::prelude::*;
use crate::runtime::scene_manager::{WorldLabel, LevelEntity, SpawnId};
use crate::schema::scene_v2::{NameplateOptionsDef, NameplateFactionFilter};
use crate::schema::stats::StatMap;
use crate::capabilities::stat_display::WorldPixelBarFillMarker;
use crate::capabilities::npc::NpcAgent;

// ─── Components ───────────────────────────────────────────────────────────────

/// Marker inserted on any spawned entity that may receive a nameplate widget.
/// Spawned by scene_loader at entity creation; `nameplate_setup_system` reacts
/// to newly added `NameplateTag` entities to create the anchor + children.
#[derive(Component, Clone)]
pub struct NameplateTag {
    /// Resolved display name (from `PrefabDef.display_name` or prefab key).
    pub display_name: String,
    /// `PrefabDef.nameplate` override.
    /// `None` = honour scene `show_nameplates` + `faction_filter`.
    /// `Some(true)` = always show (bypasses faction filter; respects `max_distance`).
    /// `Some(false)` = never show.
    pub prefab_override: Option<bool>,
}

/// Stores the entity ID of the nameplate anchor (`WorldLabel`) spawned for a world entity.
/// Attached to the same entity that carries `NameplateTag`.
#[derive(Component)]
pub struct NameplateAnchor(pub Entity);

/// Marker on the anchor entity itself so `nameplate_cleanup_system` can locate it via
/// `WorldLabel.tracked_entity` after the tracked entity is despawned.
/// The anchor is intentionally unparented (not a child of the tracked entity) to avoid
/// inheriting animation/scale transforms; this means `despawn` on the tracked
/// entity does NOT remove the anchor — cleanup must be explicit.
#[derive(Component)]
pub struct NameplateAnchorWidget;

// ─── Resource ─────────────────────────────────────────────────────────────────

/// Populated from `GameSceneV2` on scene load; cleared on `LoadScene`.
/// Consumed by both `nameplate_setup_system` and `nameplate_visibility_system`.
#[derive(Resource, Default)]
pub struct NameplateSceneConfig {
    pub enabled: bool,
    pub options: Option<NameplateOptionsDef>,
}

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Reacts to newly tagged entities (`Added<NameplateTag>`) and spawns the nameplate
/// anchor (WorldLabel) + Text2d name line + pixel stat-bar quads as Bevy children.
/// Works for both scene-placed and dynamically spawned (`Action::Spawn`) entities.
pub fn nameplate_setup_system(
    mut commands: Commands,
    config: Res<NameplateSceneConfig>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut color_materials: Option<ResMut<Assets<ColorMaterial>>>,
    new_entities: Query<(Entity, &NameplateTag, Option<&StatMap>, Option<&SpawnId>), Added<NameplateTag>>,
) {
    let (Some(meshes), Some(color_materials)) = (meshes.as_deref_mut(), color_materials.as_deref_mut()) else { return };

    // No options = nameplate system not configured; but we still need to handle
    // per-prefab Some(true) overrides even when the scene has show_nameplates: false.
    let opts = config.options.as_ref();

    for (entity, tag, stat_map, spawn_id) in new_entities.iter() {
        // Skip if explicitly suppressed.
        if tag.prefab_override == Some(false) { continue; }
        // Skip if scene disabled AND no per-prefab override.
        if !config.enabled && tag.prefab_override != Some(true) { continue; }

        let offset_v3 = opts.map(|o| {
            let (x, y, z) = o.offset;
            Vec3::new(x, y, z)
        }).unwrap_or(Vec3::new(0.0, 2.4, 0.0));
        let name_font_size = opts.map(|o| o.name_font_size).unwrap_or(14.0);
        let (nr, ng, nb, na) = opts.map(|o| o.name_color).unwrap_or((0.95, 0.95, 0.95, 1.0));
        let text_shadow = opts.map(|o| o.text_shadow).unwrap_or(true);
        let bar_w = opts.map(|o| o.bar_width).unwrap_or(100.0);
        let bar_h = opts.map(|o| o.bar_height).unwrap_or(6.0);
        let bar_spacing = opts.map(|o| o.bar_spacing).unwrap_or(9.0);
        let stat_bars = opts.map(|o| o.stat_bars.as_slice()).unwrap_or(&[]);

        // Invisible WorldLabel anchor — `world_label_screen_pos_system` projects it
        // to screen space every frame; all children follow via the Bevy hierarchy.
        let anchor = commands.spawn((
            Name::new(format!("NameplateAnchor: {}", tag.display_name)),
            Transform::default(),
            Visibility::Hidden,
            WorldLabel {
                world_pos: Vec3::ZERO,
                tracked_entity: Some(entity),
                offset: offset_v3,
                base_font_size: 1.0,
                depth_scale: None,
                screen_offset: Vec2::ZERO,
            },
            LevelEntity,
            NameplateAnchorWidget,
        )).id();

        // Drop-shadow text (slightly offset behind the main name).
        if text_shadow {
            let shadow = commands.spawn((
                Text2d::new(tag.display_name.clone()),
                TextFont { font_size: name_font_size, ..default() },
                TextColor(Color::srgba(0.0, 0.0, 0.0, na * 0.7)),
                Transform::from_xyz(1.0, -1.0, 9.0),
                LevelEntity,
            )).id();
            commands.entity(anchor).add_child(shadow);
        }

        // Main name text.
        let name_text = commands.spawn((
            Text2d::new(tag.display_name.clone()),
            TextFont { font_size: name_font_size, ..default() },
            TextColor(Color::srgba(nr, ng, nb, na)),
            Transform::from_xyz(0.0, 0.0, 10.0),
            LevelEntity,
        )).id();
        commands.entity(anchor).add_child(name_text);

        // Pixel stat bars — spawned below the name line.
        // y=0 is the name center; bars stack downward (negative y in screen space).
        let name_half = name_font_size * 0.5;
        let bar_gap_from_name = 4.0; // pixels between name baseline and first bar

        for (i, bar_def) in stat_bars.iter().enumerate() {
            // Resolve {self} in the stat key using the entity's spawn ID.
            let resolved_key = if let Some(sid) = spawn_id {
                bar_def.stat_key.replace("{self}", &sid.0)
            } else {
                bar_def.stat_key.clone()
            };

            // Check if the entity actually has this stat; skip silently when absent.
            let has_stat = stat_map.is_some_and(|sm| {
                let stat_name = resolved_key.find('.').map(|p| &resolved_key[p + 1..]).unwrap_or(&resolved_key);
                sm.0.contains_key(stat_name)
            });
            if !has_stat { continue; }

            let (fr, fg, fb, fa) = bar_def.fill_color;
            let (bgr, bgg, bgb, bga) = bar_def.bg_color;
            let bar_y = -(name_half + bar_gap_from_name + bar_h * 0.5 + i as f32 * (bar_h + bar_spacing));

            // Background quad — full bar width, static.
            let bg_child = commands.spawn((
                Name::new(format!("NameplateBarBg[{}]: {}", i, resolved_key)),
                Mesh2d(meshes.add(Rectangle::new(bar_w, bar_h))),
                MeshMaterial2d(color_materials.add(ColorMaterial::from(Color::srgba(bgr, bgg, bgb, bga)))),
                Transform::from_xyz(0.0, bar_y, 2.0),
                LevelEntity,
            )).id();
            commands.entity(anchor).add_child(bg_child);

            // Fill quad — width=1 mesh scaled per frame by `world_pixel_bar_update_system`.
            let fill_child = commands.spawn((
                Name::new(format!("NameplateBarFill[{}]: {}", i, resolved_key)),
                Mesh2d(meshes.add(Rectangle::new(1.0, bar_h))),
                MeshMaterial2d(color_materials.add(ColorMaterial::from(Color::srgba(fr, fg, fb, fa)))),
                Transform::from_xyz(-bar_w / 2.0, bar_y, 3.0)
                    .with_scale(Vec3::new(0.0, 1.0, 1.0)),
                WorldPixelBarFillMarker {
                    stat_key: resolved_key,
                    full_width: bar_w,
                    fill_color: bar_def.fill_color,
                    color_bands: vec![],
                },
                LevelEntity,
            )).id();
            commands.entity(anchor).add_child(fill_child);
        }

        // Attach the anchor reference to the world entity so visibility_system can find it.
        commands.entity(entity).insert(NameplateAnchor(anchor));
    }
}

/// Runs every frame after `world_label_screen_pos_system`.
/// Forces the nameplate anchor to `Hidden` when the entity fails distance or faction
/// criteria. Leaves visibility untouched (Visible/Hidden) when it should be shown,
/// so `world_label_screen_pos_system` handles the on-/off-screen toggle.
pub fn nameplate_visibility_system(
    config: Res<NameplateSceneConfig>,
    camera_q: Query<&GlobalTransform, With<Camera3d>>,
    entity_q: Query<(Entity, &NameplateTag, &NameplateAnchor, &GlobalTransform)>,
    npc_q: Query<(), With<NpcAgent>>,
    mut vis_q: Query<&mut Visibility>,
) {
    let Some(opts) = &config.options else { return };
    let Ok(cam_gt) = camera_q.single() else { return };
    let cam_pos = cam_gt.translation();

    for (entity, tag, anchor, gt) in entity_q.iter() {
        let should_hide = if tag.prefab_override == Some(false) {
            true
        } else if tag.prefab_override == Some(true) {
            // Bypasses faction filter; respects max_distance only.
            (gt.translation() - cam_pos).length() > opts.max_distance
        } else {
            // Apply faction filter then distance.
            let passes_faction = match opts.faction_filter {
                NameplateFactionFilter::HostileOnly  => npc_q.contains(entity),
                NameplateFactionFilter::FriendlyOnly => !npc_q.contains(entity),
                NameplateFactionFilter::All          => true,
            };
            !passes_faction || (gt.translation() - cam_pos).length() > opts.max_distance
        };

        if should_hide {
            if let Ok(mut vis) = vis_q.get_mut(anchor.0) {
                if *vis != Visibility::Hidden {
                    *vis = Visibility::Hidden;
                }
            }
        }
        // When should_hide=false, world_label_screen_pos_system (runs before this system via
        // the `.after()` ordering in lib.rs) manages Visible/Hidden based on viewport clipping.
        // IMPORTANT: this ordering contract is load-bearing — reordering breaks the two-writer
        // contract where this system is the final authority on "force-hide due to policy".
    }
}

/// Despawns orphaned nameplate anchor entities when their tracked world entity is removed.
/// Necessary because the anchor is intentionally unparented (so it does not inherit
/// animation/scale transforms from the tracked entity); `despawn` on the tracked
/// entity therefore does NOT remove the anchor, causing a leak in wave/respawn scenes.
pub fn nameplate_cleanup_system(
    mut commands: Commands,
    mut removed: RemovedComponents<NameplateTag>,
    anchors: Query<(Entity, &WorldLabel), With<NameplateAnchorWidget>>,
) {
    let removed_set: std::collections::HashSet<Entity> = removed.read().collect();
    if removed_set.is_empty() { return; }
    for (anchor_entity, world_label) in anchors.iter() {
        if let Some(tracked) = world_label.tracked_entity {
            if removed_set.contains(&tracked) {
                commands.entity(anchor_entity).despawn();
            }
        }
    }
}

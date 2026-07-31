use std::collections::{HashMap, HashSet};
use bevy::prelude::*;
use crate::runtime::scene_manager::{LevelEntity, LoadedPrefabCatalog, LoadedTargetIndicator, PrefabKey, ResolvedTargetIndicator, SpawnRegistry, TargetRingVisibilityMode};
use crate::capabilities::player::{CharacterController, PlayerIndex, PlayerTarget};
use crate::capabilities::camera::PLAYER_LABEL_COLORS;
use bevy::camera::visibility::RenderLayers;

/// Marks an active target-indicator ring: which world entity it tracks, and which player entity
/// it belongs to. Each player has at most one ring at a time — `owner` lets the system despawn
/// and rebuild exactly that player's ring without touching any other player's.
#[derive(Component)]
pub struct TrackingTarget {
    pub target: Entity,
    pub owner: Entity,
}

pub struct TargetIndicatorPlugin;

impl Plugin for TargetIndicatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, target_indicator_system);
    }
}

/// Resolves the indicator ring colour for `target_entity` using the three-tier precedence:
/// 1. Prefab `indicator_color` (direct RGBA override, highest priority)
/// 2. Prefab `indicator_category` looked up in `cfg.named_colors`
/// 3. Scene-level `cfg.color` fallback
///
/// Only used in single-player scenes — 2+ player scenes tint every ring by
/// `PLAYER_LABEL_COLORS` instead (see `target_indicator_system`), since a per-target colour
/// would make it impossible to tell whose ring is whose when two players target different
/// enemies at once.
fn resolve_indicator_color(
    target_entity: Entity,
    prefab_keys: &Query<&PrefabKey>,
    catalog: &LoadedPrefabCatalog,
    cfg: &ResolvedTargetIndicator,
) -> (f32, f32, f32, f32) {
    let Ok(PrefabKey(key)) = prefab_keys.get(target_entity) else {
        return cfg.color;
    };
    let Some(prefab) = catalog.0.prefabs.get(key) else {
        return cfg.color;
    };
    if let Some(c) = prefab.indicator_color {
        return c;
    }
    if let Some(cat) = prefab.indicator_category.as_deref() {
        if let Some(c) = cfg.named_colors.get(cat) {
            return *c;
        }
    }
    cfg.color
}

/// Spawns, moves, and despawns each player's own target-indicator ground ring independently.
///
/// Mesh is built once per scene (radius-independent of colour). Materials are memoised
/// per resolved RGBA colour — only a new colour seen for the first time in a scene causes
/// a new `StandardMaterial` allocation. Both caches are cleared when the scene changes.
///
/// - `LoadedTargetIndicator.is_changed()` clears and rebuilds the mesh cache.
/// - Every frame: moves each live ring to match its tracked entity's XZ position; despawns it
///   if the tracked entity is gone.
/// - `Changed<PlayerTarget>` (fires once per player whose selection actually changed this frame,
///   including the initial spawn) despawns that player's existing ring, if any, and spawns a new
///   one if the new selection is `Some`.
///
/// In a 2+ player scene, every ring is tinted by `PLAYER_LABEL_COLORS` (same palette as the
/// split-screen "P{n}" corner HUD label) instead of the usual per-target colour precedence, so
/// it's visually obvious whose ring is whose — rings are world-space objects visible in whatever
/// viewport frames them, not clipped to "only that player's own screen half". If two players
/// target the same entity, both rings render, coincident, each in its own player's colour — no
/// deduplication. Single-player scenes are unaffected: the ring keeps today's exact
/// prefab/category/scene colour precedence.
pub fn target_indicator_system(
    mut commands: Commands,
    indicator_cfg: Res<LoadedTargetIndicator>,
    registry: Res<SpawnRegistry>,
    prefab_catalog: Res<LoadedPrefabCatalog>,
    prefab_keys: Query<&PrefabKey>,
    global_transforms: Query<&GlobalTransform>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    ring_visibility: Res<TargetRingVisibilityMode>,
    existing: Query<(Entity, &TrackingTarget)>,
    mut transforms: Query<&mut Transform, With<TrackingTarget>>,
    changed_players: Query<(Entity, &PlayerTarget, Option<&PlayerIndex>), (With<CharacterController>, Changed<PlayerTarget>)>,
    all_players: Query<Entity, With<CharacterController>>,
    // Cached mesh handle (radius-driven, colour-independent). None when no indicator is configured.
    mut cached_mesh: Local<Option<Handle<Mesh>>>,
    // Memoised material handles keyed by resolved RGBA colour bits. Cleared on scene change.
    mut cached_mats: Local<HashMap<[u32; 4], Handle<StandardMaterial>>>,
) {
    // Rebuild mesh cache whenever the scene config changes; clear the material memo too.
    if indicator_cfg.is_changed() {
        *cached_mesh = indicator_cfg.0.as_ref().map(|cfg| {
            let d = cfg.radius * 2.0;
            meshes.add(Plane3d::default().mesh().size(d, d))
        });
        cached_mats.clear();
    }

    // Both despawn passes below (dead-target cleanup here, and owner-retarget replacement
    // further down) read the same `existing` query snapshot — since `Commands` are deferred,
    // an entity queued for despawn here is still visible to the second pass this same frame.
    // A ring whose target dies AND whose owner retargets in the same frame would otherwise be
    // queued for despawn twice, which Bevy handles gracefully but logs as a warning
    // ("Entity despawned: ... is invalid"). Track what's already been queued so each ring is
    // despawned at most once per frame.
    let mut despawn_queued: HashSet<Entity> = HashSet::new();

    // Move any existing ring to follow its tracked entity.
    for (indicator_entity, tracking) in &existing {
        match global_transforms.get(tracking.target) {
            Ok(gt) => {
                let p = gt.translation();
                if let Ok(mut tf) = transforms.get_mut(indicator_entity) {
                    if (tf.translation.x - p.x).abs() > 0.001
                        || (tf.translation.z - p.z).abs() > 0.001
                    {
                        tf.translation.x = p.x;
                        tf.translation.z = p.z;
                    }
                }
            }
            Err(_) => {
                if despawn_queued.insert(indicator_entity) {
                    commands.entity(indicator_entity).despawn();
                }
            }
        }
    }

    let Some(cfg) = &indicator_cfg.0 else { return };
    let Some(mesh_handle) = cached_mesh.as_ref() else { return };
    let is_multiplayer = all_players.iter().count() >= 2;

    for (player_entity, player_target, player_index) in &changed_players {
        // Despawn this player's existing ring (there's at most one) — every other player's
        // ring is untouched. Guarded by `despawn_queued` in case the dead-target cleanup pass
        // above already queued this same ring for despawn this frame.
        for (indicator_entity, tracking) in &existing {
            if tracking.owner == player_entity && despawn_queued.insert(indicator_entity) {
                commands.entity(indicator_entity).despawn();
            }
        }

        let Some(target_id) = player_target.0.as_ref() else { continue };

        let Some(&target_entity) = registry.entities.get(target_id) else {
            warn!("target_indicator: target '{}' not in spawn registry", target_id);
            continue;
        };

        let Ok(gt) = global_transforms.get(target_entity) else { continue };

        // Resolve colour: per-player tint in multiplayer, per-target precedence otherwise.
        let rgba = if is_multiplayer {
            let idx = player_index.map_or(0, |i| i.0) as usize % PLAYER_LABEL_COLORS.len();
            let tint = PLAYER_LABEL_COLORS[idx].to_srgba();
            (tint.red, tint.green, tint.blue, cfg.color.3)
        } else {
            resolve_indicator_color(target_entity, &prefab_keys, &prefab_catalog, cfg)
        };
        let color_key = [rgba.0.to_bits(), rgba.1.to_bits(), rgba.2.to_bits(), rgba.3.to_bits()];
        let mat_handle = cached_mats.entry(color_key).or_insert_with(|| {
            let texture: Handle<Image> = asset_server.load(cfg.texture_path.clone());
            materials.add(StandardMaterial {
                base_color_texture: Some(texture),
                base_color: Color::srgba(rgba.0, rgba.1, rgba.2, rgba.3),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                depth_bias: 64.0,
                double_sided: true,
                cull_mode: None,
                ..default()
            })
        }).clone();

        let p = gt.translation();
        let mut ring = commands.spawn((
            Name::new(format!("TargetIndicator:{}", target_id)),
            Mesh3d(mesh_handle.clone()),
            MeshMaterial3d(mat_handle),
            Transform::from_translation(Vec3::new(p.x, cfg.offset_y, p.z)),
            TrackingTarget { target: target_entity, owner: player_entity },
            LevelEntity,
        ));
        // Restrict this ring to only its owning player's viewport when opted in — invisible to
        // every camera except the one carrying this same reserved layer (see
        // `spawn_players_and_camera`'s split-camera spawn sites and `spawn_party_orbit_camera`'s
        // layer union). Layer index matches `PLAYER_LABEL_COLORS`' own player_index scheme.
        if *ring_visibility == TargetRingVisibilityMode::OwnViewportOnly {
            let idx = player_index.map_or(0, |i| i.0);
            ring.insert(RenderLayers::layer(crate::capabilities::camera::ring_layer_for_player(idx)));
        }
    }
}

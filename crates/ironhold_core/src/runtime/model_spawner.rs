
//! Centralized model spawner that always applies per-asset fixups from ProjectConfig.
//! Bevy 0.18: SceneRoot host must include Transform + Visibility.
use bevy::prelude::*;
use std::collections::HashMap;
use crate::schema::project::TransformFix;
use super::scene_manager::LevelEntity;

pub struct SpawnedModel { pub parent: Entity, pub child: Entity }

#[derive(Resource, Default)]
pub struct ModelSpawner;

impl ModelSpawner {
    pub fn spawn_instance(
        &self,
        commands: &mut Commands,
        asset_server: &AssetServer,
        fixes: &HashMap<String, TransformFix>,
        path: String,
        parent_tf: Transform,
    ) -> SpawnedModel {
        let fix = fixes
            .get(&path)
            .or_else(|| path.split('#').next().and_then(|base| fixes.get(base)))
            .cloned()
            .unwrap_or_default();

        let name = path.split('/').last().unwrap_or(&path).to_string();
        let parent = commands
            .spawn((
                Name::new(name),
                parent_tf,
                Visibility::default(),
                LevelEntity,
            ))
            .id();

        let fix_t = Vec3::new(fix.pivot_offset.0, fix.pivot_offset.1, fix.pivot_offset.2);
        let fix_r = Quat::from_euler(
            EulerRot::YXZ,
            fix.rotation_deg.1.to_radians(),
            fix.rotation_deg.0.to_radians(),
            fix.rotation_deg.2.to_radians(),
        );
        let fix_s = Vec3::new(fix.scale.0, fix.scale.1, fix.scale.2);

        let child = commands
            .spawn((
                Name::new("Model Scene Root"),
                SceneRoot(asset_server.load(path)),
                Transform { translation: fix_t, rotation: fix_r, scale: fix_s },
                Visibility::default(),
            ))
            .id();

        commands.entity(parent).add_child(child);
        SpawnedModel { parent, child }
    }
}

use bevy::prelude::*;
use bevy::gltf::Gltf;
use std::{
    collections::{
        HashMap,
        HashSet,
    },
};

use crate::capabilities::animation_resolver::AnimationPolicyComponent;

#[derive(Component)]
pub struct AnimationController {
    pub current: String,
    pub last_played: String,
    pub gltf_path: String,
    pub gltf_handle: Handle<Gltf>,
    pub node_indices: HashMap<String, AnimationNodeIndex>,
    pub graph_initialized: bool,
}

pub fn animation_playback_system(
    mut commands: Commands,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut controller_query: Query<(Entity, &mut AnimationController, &AnimationPolicyComponent)>,
    mut player_query: Query<&mut AnimationPlayer>,
    children_query: Query<&Children>,
) {
    for (entity, mut controller, policy_comp) in &mut controller_query {
        // 1. Initialize Graph if not done and GLTF is ready
        if !controller.graph_initialized {
            if let Some(gltf) = gltfs.get(&controller.gltf_handle) {
                let policy = &policy_comp.0;
                let mut graph = AnimationGraph::new();
                let mut indices = HashMap::new();

                // Collect unique clip names we may want to play.
                let mut clip_names: HashSet<String> = HashSet::new();
                clip_names.insert(policy.base.idle.clone());
                clip_names.insert(policy.base.walk.clone());
                clip_names.insert(policy.base.run.clone());
                for v in policy.clips.values() {
                    clip_names.insert(v.clone());
                }
                for ov in &policy.overrides {
                    clip_names.insert(ov.clip.clone());
                }

                // Add each named clip into the graph (if present in GLTF).
                for name in clip_names {
                    if let Some(clip) = gltf.named_animations.get(&*name) {
                        let index = graph.add_clip(clip.clone(), 1.0, graph.root);
                        indices.insert(name, index);
                    }
                }

                let graph_handle = graphs.add(graph);

                // Find entity with AnimationPlayer to insert Graph handle
                if let Some(player_ent) = find_player_entity_recursive(entity, &player_query, &children_query) {
                    commands.entity(player_ent).insert(AnimationGraphHandle(graph_handle));
                    controller.node_indices = indices;
                    controller.graph_initialized = true;
                    info!("Animation Graph Initialized!");
                }
            }
        }

        // 2. Handle Playback
        if controller.graph_initialized && controller.current != controller.last_played {
            if let Some(player_ent) = find_player_entity_recursive(entity, &player_query, &children_query) {
                if let Ok(mut player) = player_query.get_mut(player_ent) {
                    if let Some(&index) = controller.node_indices.get(&controller.current) {
                        // For now: always repeat; one-shot behavior is handled by resolver expiry.
                        player.play(index).repeat();
                        controller.last_played = controller.current.clone();
                    } else {
                        warn!("No node index for requested animation: {}", controller.current);
                    }
                }
            }
        }
    }
}

fn find_player_entity_recursive(
    entity: Entity,
    player_query: &Query<&mut AnimationPlayer>,
    children_query: &Query<&Children>,
) -> Option<Entity> {
    if player_query.contains(entity) {
        return Some(entity);
    }

    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            if let Some(found) = find_player_entity_recursive(child, player_query, children_query) {
                return Some(found);
            }
        }
    }

    None
}

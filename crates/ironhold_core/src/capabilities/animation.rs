use bevy::prelude::*;
use bevy::reflect::Reflect;
use bevy::gltf::Gltf;
use std::time::Duration;
use std::{
    collections::{
        HashMap,
        HashSet,
    },
};

use crate::capabilities::animation_resolver::AnimationPolicyComponent;

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct AnimationController {
    pub current: String,
    pub last_played: String,
    pub gltf_path: String,
    pub gltf_handle: Handle<Gltf>,
    pub node_indices: HashMap<String, AnimationNodeIndex>,
    pub graph_initialized: bool,
    pub transition_ms: u64,
    pub should_loop: bool,
}

pub fn animation_playback_system(
    mut commands: Commands,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut controller_query: Query<(Entity, &mut AnimationController, &AnimationPolicyComponent)>,
    player_marker_query: Query<(), With<AnimationPlayer>>,
    mut player_query: Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
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
                clip_names.insert(policy.base.jump_loop.clone());
                for v in policy.clips.values() {
                    clip_names.insert(v.clone());
                }
                for ov in &policy.overrides {
                    clip_names.insert(ov.clip.clone());
                }

                // Add each named clip into the graph (if present in GLTF).
                for name in &clip_names {
                    if let Some(clip) = gltf.named_animations.get(name.as_str()) {
                        let index = graph.add_clip(clip.clone(), 1.0, graph.root);
                        indices.insert(name.clone(), index);
                    }
                }

                // Warn about clips declared in the policy that don't exist in the GLB.
                let mut missing: Vec<&str> = clip_names
                    .iter()
                    .filter(|n| !indices.contains_key(n.as_str()))
                    .map(|s| s.as_str())
                    .collect();
                if !missing.is_empty() {
                    missing.sort();
                    let mut available: Vec<&str> =
                        gltf.named_animations.keys().map(|s| s.as_ref()).collect();
                    available.sort();
                    warn!(
                        "AnimationPolicy: {} clip(s) not found in \"{}\": [{}]. Available clips: [{}]",
                        missing.len(),
                        controller.gltf_path,
                        missing.join(", "),
                        available.join(", ")
                    );
                }

                let graph_handle = graphs.add(graph);

                // Find entity with AnimationPlayer to insert Graph handle
                if let Some(player_ent) = find_player_entity_recursive(entity, &player_marker_query, &children_query) {
                    commands.entity(player_ent).insert((AnimationGraphHandle(graph_handle), AnimationTransitions::new()));
                    controller.node_indices = indices;
                    controller.graph_initialized = true;
                    info!("Animation Graph Initialized!");
                }
            }
        }

        // 2. Handle Playback
        if controller.graph_initialized && controller.current != controller.last_played {
            if let Some(player_ent) = find_player_entity_recursive(entity, &player_marker_query, &children_query) {
                if let Ok((mut player, maybe_transitions)) = player_query.get_mut(player_ent) {
                    if let Some(&index) = controller.node_indices.get(&controller.current) {
                        let duration = if controller.transition_ms == 0 { Duration::ZERO } else { Duration::from_millis(controller.transition_ms) };
                        if let Some(mut transitions) = maybe_transitions {
                            let active_anim = transitions.play(&mut player, index, duration);
                            if controller.should_loop {
                                active_anim.repeat();
                            }
                            // Only commit last_played when AnimationTransitions is ready.
                            // AnimationGraphHandle + AnimationTransitions are inserted via deferred
                            // commands on the same frame the graph is initialized, so they won't
                            // be present until the next frame. Skipping last_played here causes
                            // a retry next frame via the transitions path.
                            controller.last_played = controller.current.clone();
                        } else {
                            // AnimationTransitions not yet applied (deferred command still pending).
                            // Don't update last_played — retry next frame when transitions exist.
                        }
                    } else {
                        warn!("No node index for requested animation: {}", controller.current);
                        // Avoid retrying every frame for a clip that will never exist.
                        controller.last_played = controller.current.clone();
                    }
                }
            }
        }
    }
}

fn find_player_entity_recursive(
    entity: Entity,
    player_query: &Query<(), With<AnimationPlayer>>,
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


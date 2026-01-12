use bevy::prelude::*;
use bevy::gltf::Gltf;
use std::collections::HashMap;
use crate::schema::player::AnimationMap;

#[derive(Component)]
pub struct AnimationController {
    pub animations: AnimationMap,
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
    mut controller_query: Query<(Entity, &mut AnimationController)>,
    mut player_query: Query<&mut AnimationPlayer>,
    children_query: Query<&Children>,
) {
    for (entity, mut controller) in &mut controller_query {
        // 1. Initialize Graph if not done and GLTF is ready
        if !controller.graph_initialized {
            if let Some(gltf) = gltfs.get(&controller.gltf_handle) {
                let mut graph = AnimationGraph::new();
                let mut indices = HashMap::new();
                
                // Collect all animations from the map
                let anim_names = vec![
                    controller.animations.idle.clone(),
                    controller.animations.walk.clone(),
                    controller.animations.run.clone(),
                    controller.animations.jump_enter.clone(),
                    controller.animations.jump_loop.clone(),
                    controller.animations.jump_exit.clone(),
                    controller.animations.death.clone(),
                    controller.animations.dance.clone(),
                    controller.animations.crouch_idle.clone(),
                    controller.animations.crouch_forward.clone(),
                    controller.animations.roll.clone(),
                ];
                
                for name in anim_names {
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
                    println!("Animation Graph Initialized!");
                }
            }
        }
        
        // 2. Handle Playback
        if controller.graph_initialized && controller.current != controller.last_played {
            if let Some(player_ent) = find_player_entity_recursive(entity, &player_query, &children_query) {
                if let Ok(mut player) = player_query.get_mut(player_ent) {
                    if let Some(&index) = controller.node_indices.get(&controller.current) {
                        player.play(index).repeat();
                        controller.last_played = controller.current.clone();
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

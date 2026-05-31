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
    /// Entity that held `AnimationPlayer` on the last successful play; used to detect
    /// hierarchy changes that would silently invalidate the animation graph.
    pub last_player_entity: Option<Entity>,
}

pub fn animation_playback_system(
    mut commands: Commands,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut controller_query: Query<(Entity, &mut AnimationController, &AnimationPolicyComponent)>,
    player_marker_query: Query<(), With<AnimationPlayer>>,
    mut player_query: Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
    children_query: Query<&Children>,
    names: Query<&Name>,
) {
    for (entity, mut controller, policy_comp) in &mut controller_query {
        let entity_name = names.get(entity).map(|n| n.as_str()).unwrap_or("<unnamed>");

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
                        "[{}] AnimationPolicy: {} clip(s) not found in \"{}\": [{}]. Available: [{}]",
                        entity_name,
                        missing.len(),
                        controller.gltf_path,
                        missing.join(", "),
                        available.join(", ")
                    );
                }

                let graph_handle = graphs.add(graph);

                // Find entity with AnimationPlayer to insert Graph handle.
                // Returns None if the GLTF scene children haven't been spawned yet
                // (SpawnScene runs after Update — retry next frame).
                match find_player_entity_recursive(entity, &player_marker_query, &children_query) {
                    Some(player_ent) => {
                        commands.entity(player_ent).insert((
                            AnimationGraphHandle(graph_handle),
                            AnimationTransitions::new(),
                        ));
                        controller.node_indices = indices;
                        controller.graph_initialized = true;
                        controller.last_player_entity = Some(player_ent);
                        info!(
                            "[{}] Animation graph ready: {} clip(s) mapped, starting clip: {:?}, AnimationPlayer: {:?}",
                            entity_name,
                            controller.node_indices.len(),
                            controller.current,
                            player_ent,
                        );
                    }
                    None => {
                        // GLTF scene hierarchy not yet spawned (SpawnScene runs after Update).
                        // graph_handle drops here — graph is rebuilt on the next successful attempt.
                        debug!(
                            "[{}] Graph init deferred: AnimationPlayer not yet in hierarchy (GLTF: {})",
                            entity_name, controller.gltf_path
                        );
                    }
                }
            }
        }

        // Entity staleness check — runs every frame once the graph is initialized,
        // regardless of whether the current clip is changing. This is critical for
        // entities whose animation never changes (e.g. NPCs always playing "idle"):
        // the current != last_played guard on step 2 is never true for them, so the
        // entity change must be caught here instead.
        //
        // Bevy's SceneSpawner replaces the GLTF hierarchy when sub-assets finish loading
        // (common on WASM where textures arrive slightly after the initial scene spawn).
        // The old AnimationPlayer entity is despawned; a new entity appears in the same
        // hierarchy position. Our AnimationGraphHandle and AnimationTransitions are on
        // the OLD entity — the new one has neither. Reset graph_initialized so step 1
        // re-inserts them on the new entity next frame.
        //
        // Fast path: if the cached entity still has AnimationPlayer, skip the O(tree_depth)
        // hierarchy walk entirely. Fall back to the full walk only on a cache miss
        // (the cached entity was despawned/replaced — the rare GLTF re-spawn case).
        if controller.graph_initialized {
            if let Some(cached) = controller.last_player_entity {
                if !player_marker_query.contains(cached) {
                    // Cache miss: cached entity no longer has AnimationPlayer — do the full walk.
                    match find_player_entity_recursive(entity, &player_marker_query, &children_query) {
                        Some(found) => {
                            warn!(
                                "[{}] AnimationPlayer entity changed {:?} → {:?} — GLTF scene re-spawned. \
                                Resetting animation graph for re-initialization next frame.",
                                entity_name, controller.last_player_entity, found
                            );
                            controller.graph_initialized = false;
                            controller.last_player_entity = None;
                            controller.last_played = String::new();
                            continue;
                        }
                        None => {
                            warn!(
                                "[{}] AnimationPlayer entity lost after graph init — resetting for re-initialization",
                                entity_name
                            );
                            controller.graph_initialized = false;
                            controller.last_player_entity = None;
                            controller.last_played = String::new();
                            continue;
                        }
                    }
                }
                // else: cache hit — cached entity is still valid, no tree walk needed.
            }
            // If last_player_entity is None (shouldn't happen outside tests), skip
            // the staleness check and let step 2's fallback resolve it.
        }

        // 2. Handle Playback
        if controller.graph_initialized && controller.current != controller.last_played {
            // Fast path: use the cached entity (staleness check above confirmed it's valid).
            // Fall back to the recursive walk only when last_player_entity is None, which
            // shouldn't happen in production (graph init always sets both together) but can
            // occur in tests that construct synthetic controller state.
            let maybe_player = controller.last_player_entity
                .or_else(|| find_player_entity_recursive(entity, &player_marker_query, &children_query));
            if let Some(player_ent) = maybe_player {
                if let Ok((mut player, maybe_transitions)) = player_query.get_mut(player_ent) {
                    if let Some(&index) = controller.node_indices.get(&controller.current) {
                        let duration = if controller.transition_ms == 0 { Duration::ZERO } else { Duration::from_millis(controller.transition_ms) };
                        if let Some(mut transitions) = maybe_transitions {
                            controller.last_player_entity = Some(player_ent);
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
                            // AnimationTransitions not yet applied (normal 1-frame deferred window
                            // after graph init). Don't update last_played — retry next frame.
                            debug!("[{}] Waiting for AnimationTransitions (deferred — retrying next frame)", entity_name);
                        }
                    } else {
                        // Clip name is not in node_indices. Log the warning, silence per-frame
                        // spam by advancing last_played, then reset current to idle so the
                        // resolver can recover on the next frame. Without the reset, current
                        // == last_played forever — the "permanent trap" that freezes animations
                        // in T-pose for the rest of the session.
                        let available_keys: Vec<&str> = controller.node_indices.keys().map(|s| s.as_str()).collect();
                        warn!(
                            "[{}] No node index for animation {:?} — available: [{}]. Resetting to idle.",
                            entity_name,
                            controller.current,
                            available_keys.join(", ")
                        );
                        controller.last_played = controller.current.clone();
                        controller.current = policy_comp.0.base.idle.clone();
                    }
                } else {
                    warn!(
                        "[{}] player_query.get_mut({:?}) returned Err — AnimationPlayer entity \
                        found by hierarchy search is not accessible via the playback query",
                        entity_name, player_ent
                    );
                }
            }
            // Note: last_player_entity is always Some here — staleness check above
            // resets graph_initialized (and continues) whenever the cached entity is lost.
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


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

use crate::capabilities::animation_resolver::{AnimationPolicyComponent, ActiveOverride};

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct AnimationController {
    pub current: String,
    pub last_played: String,
    pub gltf_path: String,
    pub gltf_handle: Handle<Gltf>,
    /// Extra animation-pack GLBs loaded from `AnimationPolicy.animation_sources`.
    /// Graph init waits until all of these are loaded before building the merged graph.
    pub source_handles: Vec<Handle<Gltf>>,
    pub node_indices: HashMap<String, AnimationNodeIndex>,
    /// The `AnimationGraph` asset `node_indices` was built against. Stored here (synchronously
    /// updated alongside `node_indices`) rather than read from the entity's `AnimationGraphHandle`
    /// component, because that component is inserted via a *deferred* command — on the exact
    /// frame a re-init happens (e.g. `animation.rs`'s GLTF-hierarchy-respawn recovery path),
    /// `node_indices` is already the fresh map but the component still reflects the OLD graph
    /// until commands flush. Reading the component for a duration lookup on that transitional
    /// frame pairs a fresh index against a stale graph, which is a mismatch a `HashSet`'s
    /// non-deterministic iteration order between builds can turn into a completely wrong clip
    /// duration (found by this feature's own test suite, not by inspection).
    pub graph_handle: Option<Handle<AnimationGraph>>,
    pub graph_initialized: bool,
    pub transition_ms: u64,
    pub should_loop: bool,
    /// Entity that held `AnimationPlayer` on the last successful play; used to detect
    /// hierarchy changes that would silently invalidate the animation graph.
    pub last_player_entity: Option<Entity>,
    /// Set by `animation_resolver_system` whenever it accepts a queued request that carries a
    /// `seek_fraction`/`freeze` (`ActiveOverride`'s durable fields) — even one naming the
    /// clip that's already current. Playback normally only re-triggers on
    /// `current != last_played`, which is a no-op for "re-seek the same clip to a different
    /// fraction" (exactly the `dynamic_animation_control` demo's QA matrix). This flag forces
    /// that replay; cleared by `animation_playback_system` once applied.
    pub pending_seek: bool,
    /// Set to `true` (with the entity spawned `Visibility::Hidden`, see
    /// `entity_spawner.rs::spawn_prefab_instance`) for every entity that gets an `animation_policy`
    /// — hiding happens at spawn itself, before the policy RON has even loaded, because
    /// `bevy_scene`'s `SpawnScene` step instantiates the (often-cached) GLTF hierarchy in that same
    /// spawn frame, well before anything in this engine gets a chance to react. Left hidden that
    /// long would show a raw GLTF bind/rest pose for at least one real rendered frame — found via a
    /// real playtest as a corpse briefly appearing to "stand" before settling into its frozen death
    /// pose, separate from (and only visible after fixing) an earlier looping bug this feature also
    /// fixed. `animation_playback_system` reveals (`Visibility::Inherited`) the moment a pose is
    /// actually confirmed applied — which for `AnimationPolicy.initial_override` entities (e.g. a
    /// corpse's frozen death pose) is the seek+freeze application, and for every other entity is
    /// simply "no override, fall through to `base.idle`" the instant the policy loads. A bounded
    /// failsafe (`awaiting_reveal_since`) force-reveals regardless if neither ever happens.
    pub awaiting_reveal: bool,
    /// Wall-clock (`Time::elapsed_secs()`) timestamp of when `awaiting_reveal` was last set —
    /// used by `animation_playback_system`'s failsafe to force-reveal an entity that never gets
    /// a confirmed pose (e.g. a broken `animation_policy` path, or a model with no
    /// `AnimationPlayer` in its hierarchy). `None` until `animation_policy_loader_system` gets a
    /// chance to stamp it (spawn itself has no `Time` access), so the failsafe's window doesn't
    /// start ticking until then — acceptable since that gap is bounded by the same async RON
    /// fetch this whole mechanism exists to hide.
    pub awaiting_reveal_since: Option<f32>,
}

pub fn animation_playback_system(
    mut commands: Commands,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    clips: Res<Assets<AnimationClip>>,
    mut controller_query: Query<(Entity, &mut AnimationController, &AnimationPolicyComponent, &ActiveOverride)>,
    player_marker_query: Query<(), With<AnimationPlayer>>,
    mut player_query: Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
    children_query: Query<&Children>,
    names: Query<&Name>,
    time: Res<Time>,
) {
    /// If an entity is still hidden waiting for a confirmed pose this long after
    /// `awaiting_reveal_since` was stamped, reveal it anyway — an incorrect pose is preferable
    /// to a permanently invisible entity (e.g. a broken `animation_policy`/model reference, or
    /// a GLB with no `AnimationPlayer` in its hierarchy).
    const REVEAL_FAILSAFE_SECS: f32 = 5.0;

    for (entity, mut controller, policy_comp, active_override) in &mut controller_query {
        let entity_name = names.get(entity).map(|n| n.as_str()).unwrap_or("<unnamed>");

        if controller.awaiting_reveal {
            if let Some(since) = controller.awaiting_reveal_since {
                if time.elapsed_secs() - since > REVEAL_FAILSAFE_SECS {
                    warn!(
                        "[{}] awaiting_reveal exceeded {}s with no confirmed pose — revealing anyway to avoid a permanently invisible entity",
                        entity_name, REVEAL_FAILSAFE_SECS
                    );
                    controller.awaiting_reveal = false;
                    controller.awaiting_reveal_since = None;
                    commands.entity(entity).insert(Visibility::Inherited);
                }
            }
        }

        // 1. Initialize Graph if not done and GLTF is ready
        if !controller.graph_initialized {
            if let Some(gltf) = gltfs.get(&controller.gltf_handle) {
                // Wait until every animation-source GLB is also loaded.
                if controller.source_handles.iter().any(|h| gltfs.get(h).is_none()) {
                    continue;
                }

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

                // Build merged named_animations: model GLB first, then each source in order.
                // Later entries win on duplicate clip names.
                let mut merged = gltf.named_animations.clone();
                for source_handle in &controller.source_handles {
                    if let Some(src) = gltfs.get(source_handle) {
                        for (name, clip) in &src.named_animations {
                            if merged.contains_key(name) {
                                warn!(
                                    "[{}] animation_sources: clip '{}' overrides an earlier definition",
                                    entity_name, name
                                );
                            }
                            merged.insert(name.clone(), clip.clone());
                        }
                    }
                }

                // Add each named clip into the graph (if present in any source).
                for name in &clip_names {
                    if let Some(clip) = merged.get(name.as_str()) {
                        let index = graph.add_clip(clip.clone(), 1.0, graph.root);
                        indices.insert(name.clone(), index);
                    }
                }

                // Warn about clips declared in the policy that don't exist in any GLB.
                let mut missing: Vec<&str> = clip_names
                    .iter()
                    .filter(|n| !indices.contains_key(n.as_str()))
                    .map(|s| s.as_str())
                    .collect();
                if !missing.is_empty() {
                    missing.sort();
                    let mut available: Vec<&str> = merged.keys().map(|s| s.as_ref()).collect();
                    available.sort();
                    warn!(
                        "[{}] AnimationPolicy: {} clip(s) not found in model or sources: [{}]. Available: [{}]",
                        entity_name,
                        missing.len(),
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
                            AnimationGraphHandle(graph_handle.clone()),
                            AnimationTransitions::new(),
                        ));
                        controller.node_indices = indices;
                        controller.graph_handle = Some(graph_handle);
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
                            // The replacement hierarchy's meshes start in the GLTF's rest/bind
                            // pose until graph re-init re-applies the current pose to the new
                            // AnimationPlayer entity — re-arm the same hide/reveal guard used on
                            // initial spawn (see AnimationController.awaiting_reveal) so this
                            // mid-life re-spawn doesn't flash the bind pose too.
                            controller.awaiting_reveal = true;
                            controller.awaiting_reveal_since = Some(time.elapsed_secs());
                            info!(
                                "[DIAG] [{}] re-hiding {:?} — GLTF hierarchy re-spawned, pose not yet reapplied",
                                entity_name, entity
                            );
                            commands.entity(entity).insert(Visibility::Hidden);
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
                            controller.awaiting_reveal = true;
                            controller.awaiting_reveal_since = Some(time.elapsed_secs());
                            info!(
                                "[DIAG] [{}] re-hiding {:?} — AnimationPlayer lost, pose not yet reapplied",
                                entity_name, entity
                            );
                            commands.entity(entity).insert(Visibility::Hidden);
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
        if controller.graph_initialized
            && (controller.current != controller.last_played || controller.pending_seek)
        {
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

                            // A frozen (paused) previous clip is invisible to
                            // AnimationTransitions::play's own fade-out guard (it explicitly
                            // skips creating a transition for a paused outgoing animation), so
                            // it would otherwise never decay out of the player's active
                            // animations and stay permanently blended at full weight against
                            // whatever plays next. Resume it first so the normal fade-out path
                            // applies exactly as it would for any other clip switch.
                            if let Some(&prev_index) = controller.node_indices.get(&controller.last_played) {
                                if let Some(prev_anim) = player.animation_mut(prev_index) {
                                    if prev_anim.is_paused() {
                                        prev_anim.resume();
                                    }
                                }
                            }

                            info!(
                                "[DIAG] [{}] playing clip={:?} should_loop={} seek_fraction={:?} frozen={} awaiting_reveal={} (last_played was {:?})",
                                entity_name, controller.current, controller.should_loop,
                                active_override.seek_fraction, active_override.frozen, controller.awaiting_reveal, controller.last_played
                            );
                            let active_anim = transitions.play(&mut player, index, duration);
                            // Explicit set_repeat every play, not just a conditional `.repeat()`
                            // — `AnimationPlayer::start()` (called by `transitions.play()`)
                            // reuses the existing `ActiveAnimation` entry when replaying the
                            // SAME node index (exactly what a `pending_seek` same-clip re-seek
                            // does) and its `.replay()` does not reset `repeat`. Without this
                            // being unconditional, a node previously set to `Forever` (e.g. by
                            // an earlier `should_loop: true` play) would stay stuck looping
                            // forever even after a later play sets `should_loop: false`.
                            active_anim.set_repeat(if controller.should_loop {
                                bevy::animation::RepeatAnimation::Forever
                            } else {
                                bevy::animation::RepeatAnimation::Never
                            });

                            // Seek/freeze — durable on ActiveOverride, so this reapplies on
                            // every real play (not just the first), including a later replay
                            // forced by the GLTF-hierarchy-respawn recovery path above. Use
                            // set_seek_time (not seek_to) so no animation events between the
                            // old and new time are replayed.
                            if let Some(fraction) = active_override.seek_fraction {
                                let clip_duration = controller.graph_handle.as_ref()
                                    .and_then(|h| graphs.get(h))
                                    .and_then(|g| g.get(index))
                                    .and_then(|node| match &node.node_type {
                                        AnimationNodeType::Clip(handle) => Some(handle),
                                        _ => None,
                                    })
                                    .and_then(|handle| clips.get(handle))
                                    .map(|clip| clip.duration());
                                match clip_duration {
                                    Some(d) => {
                                        active_anim.set_seek_time(fraction * d);
                                    }
                                    None => warn!(
                                        "[{}] Could not resolve duration for clip {:?} — seek to fraction {} skipped",
                                        entity_name, controller.current, fraction
                                    ),
                                }
                            }
                            // `freeze` is independent of whether a fraction was given — `freeze:
                            // true` with no `start_at_fraction` means "hold at the start (0.0),
                            // don't play at all", a legitimate hard-stop use. Previously this
                            // pause() lived inside the `seek_fraction` block above, so `freeze:
                            // true` alone silently did nothing (found by 3 independent reviews).
                            // Symmetric `resume()` covers the same-node re-seek case: switching
                            // TO a different clip is handled by the resume-before-play step
                            // above, but re-seeking the SAME already-current clip from frozen to
                            // continuing never goes through that step (last_played == current).
                            if active_override.frozen {
                                active_anim.pause();
                            } else {
                                active_anim.resume();
                            }

                            // Only commit last_played when AnimationTransitions is ready.
                            // AnimationGraphHandle + AnimationTransitions are inserted via deferred
                            // commands on the same frame the graph is initialized, so they won't
                            // be present until the next frame. Skipping last_played here causes
                            // a retry next frame via the transitions path.
                            controller.last_played = controller.current.clone();
                            controller.pending_seek = false;

                            // The pose this frame's play() call just applied is now real and
                            // fully seeked/frozen — reveal an entity that was hidden pending
                            // exactly this confirmation (see AnimationController.awaiting_reveal).
                            if controller.awaiting_reveal {
                                controller.awaiting_reveal = false;
                                controller.awaiting_reveal_since = None;
                                info!("[DIAG] [{}] revealing {:?} — pose confirmed applied", entity_name, entity);
                                commands.entity(entity).insert(Visibility::Inherited);
                            }
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


use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::schema::catalog::{NpcFaction, NpcOnPlayerNear};
use crate::capabilities::player::CharacterController;
use crate::runtime::messages::GameEvent;

// ── Runtime state enum ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum NpcState {
    /// Standing still; no waypoints configured.
    Idle,
    /// Walking between patrol waypoints.
    Patrol,
    /// Player spotted — brief pause (0.3 s) before acting.
    Alerted,
    /// Moving toward (Chase/Interact) or away from (Flee) the player.
    Chase,
    /// Player escaped; walking back to the patrol origin.
    Return,
    /// Within `approach_distance` of a friendly player.
    Interact,
}

// ── NpcAgent component ────────────────────────────────────────────────────────

#[derive(Component)]
pub struct NpcAgent {
    pub npc_id: String,
    pub faction: NpcFaction,
    pub on_player_near: NpcOnPlayerNear,
    /// Metres inside which the NPC reacts.
    pub detection_radius: f32,
    /// Metres beyond which the NPC gives up chasing.
    pub chase_radius: f32,
    /// `cos(fov_degrees / 2)` — pre-computed for fast dot-product FOV check.
    /// `-1.0` encodes 360° (always passes).
    pub fov_cos: f32,
    /// Whether a Rapier ray cast must confirm unobstructed line of sight.
    pub requires_los: bool,
    /// Stop approaching at this distance (greet / attack range).
    pub approach_distance: f32,
    pub patrol_speed: f32,
    pub chase_speed: f32,
    /// World-space patrol waypoints (converted from spawn-relative offsets at spawn).
    pub waypoints: Vec<Vec3>,
    pub current_waypoint: usize,
    pub state: NpcState,
    /// Target player entity (updated each frame while chasing).
    pub target: Option<Entity>,
    /// Seconds spent in the current state — drives the Alerted pause timer.
    pub state_timer: f32,
    /// World-space spawn origin used by the Return state to home back.
    pub origin: Vec3,
}

// ── Visibility helper ─────────────────────────────────────────────────────────

/// Returns `(player_entity, player_world_pos, distance)` for the nearest player
/// that passes the FOV and (optionally) line-of-sight checks.
fn find_nearest_visible_player<'r>(
    npc_entity: Entity,
    npc_pos: Vec3,
    npc_forward: Vec3,
    fov_cos: f32,
    requires_los: bool,
    players: &[(Entity, Vec3)],
    rapier: Option<&'r RapierContext<'r>>,
) -> Option<(Entity, Vec3, f32)> {
    let mut best: Option<(Entity, Vec3, f32)> = None;

    for &(player_entity, player_pos) in players {
        let to_player = player_pos - npc_pos;
        let dist = to_player.length();
        if dist < 0.01 { continue; }

        // ── FOV check ─────────────────────────────────────────────────────────
        // fov_cos == -1.0  →  cos(180°)  →  360° vision, always passes.
        if fov_cos > -1.0 {
            let dir_to_player = to_player / dist;
            if npc_forward.dot(dir_to_player) < fov_cos {
                continue; // outside the forward cone
            }
        }

        // ── Line-of-sight ray cast ─────────────────────────────────────────────
        if requires_los {
            if let Some(ctx) = rapier {
                let eye = npc_pos + Vec3::Y * 0.9; // approximate eye height
                let ray_dir = (to_player / dist).into();
                let hit = ctx.cast_ray(
                    eye,
                    ray_dir,
                    dist,
                    true,
                    QueryFilter::new()
                        .exclude_rigid_body(npc_entity)
                        .exclude_sensors(),
                );
                match hit {
                    // Ray reached the player — clear line of sight.
                    Some((hit_entity, _)) if hit_entity == player_entity => {}
                    // Ray hit something else (wall, prop) — blocked.
                    _ => continue,
                }
            }
        }

        if best.map(|(_, _, d)| dist < d).unwrap_or(true) {
            best = Some((player_entity, player_pos, dist));
        }
    }

    best
}

// ── Behaviour system ──────────────────────────────────────────────────────────

/// Runs every physics tick (FixedUpdate).
/// For each NPC:
///   1. Detect the nearest visible player.
///   2. Drive the NPC state machine.
///   3. Emit `GameEvent::Trigger` on state transitions so RON rules can react.
///   4. Set `Velocity.linvel` to move toward/away from the target.
pub fn npc_behavior_system(
    time: Res<Time>,
    mut npc_query: Query<(
        Entity,
        &mut NpcAgent,
        &mut Transform,
        &GlobalTransform,
        &mut Velocity,
    )>,
    player_query: Query<(Entity, &GlobalTransform), With<CharacterController>>,
    rapier_context: Option<ReadRapierContext>,
    mut game_events: MessageWriter<GameEvent>,
) {
    let rapier = rapier_context.as_ref().and_then(|rc| rc.single().ok());
    let dt = time.delta_secs();

    // Snapshot all player positions once per tick (avoids repeated query access).
    let players: Vec<(Entity, Vec3)> = player_query
        .iter()
        .map(|(e, gt)| (e, gt.translation()))
        .collect();

    for (npc_entity, mut npc, mut transform, global_tf, mut velocity) in &mut npc_query {
        let npc_pos = global_tf.translation();
        let npc_forward = Vec3::from(transform.forward());

        let visible = find_nearest_visible_player(
            npc_entity,
            npc_pos,
            npc_forward,
            npc.fov_cos,
            npc.requires_los,
            &players,
            rapier.as_ref(),
        );

        let dist_opt   = visible.map(|(_, _, d)| d);
        let in_detect  = dist_opt.map(|d| d <= npc.detection_radius).unwrap_or(false);
        let in_chase   = dist_opt.map(|d| d <= npc.chase_radius).unwrap_or(false);
        let in_approach = dist_opt.map(|d| d <= npc.approach_distance).unwrap_or(false);

        // ── State machine ──────────────────────────────────────────────────────
        let mut next_state: Option<NpcState> = None;
        let mut pending_event: Option<String> = None;

        match npc.state {
            NpcState::Idle | NpcState::Patrol => {
                if in_detect {
                    if let Some((e, _, _)) = visible { npc.target = Some(e); }
                    npc.state_timer = 0.0;
                    next_state = Some(NpcState::Alerted);
                }
            }

            NpcState::Alerted => {
                npc.state_timer += dt;
                if npc.state_timer >= 0.3 {
                    pending_event = Some(format!("npc.player_spotted:{}", npc.npc_id));
                    next_state = Some(match npc.on_player_near {
                        // Alert NPCs just acknowledge and resume — no movement.
                        NpcOnPlayerNear::Alert => {
                            if npc.waypoints.is_empty() { NpcState::Idle } else { NpcState::Patrol }
                        }
                        _ => NpcState::Chase,
                    });
                }
            }

            NpcState::Chase => {
                // Continuously refresh target while the player is visible.
                if let Some((e, _, _)) = visible { npc.target = Some(e); }

                let flee = matches!(npc.on_player_near, NpcOnPlayerNear::Flee);
                if !flee && in_approach {
                    pending_event = Some(format!("npc.player_reached:{}", npc.npc_id));
                    npc.state_timer = 0.0;
                    next_state = Some(NpcState::Interact);
                } else if !in_chase {
                    pending_event = Some(format!("npc.player_lost:{}", npc.npc_id));
                    next_state = Some(NpcState::Return);
                }
            }

            NpcState::Interact => {
                // Leave interact range → resume patrol / idle.
                if dist_opt.map(|d| d > npc.approach_distance * 1.5).unwrap_or(true) {
                    next_state = Some(if npc.waypoints.is_empty() {
                        NpcState::Idle
                    } else {
                        NpcState::Patrol
                    });
                }
            }

            NpcState::Return => {
                // Re-engage if player wanders back into range during the return walk.
                if in_detect {
                    if let Some((e, _, _)) = visible { npc.target = Some(e); }
                    npc.state_timer = 0.0;
                    next_state = Some(NpcState::Alerted);
                } else if npc_pos.distance(npc.origin) < 0.5 {
                    next_state = Some(if npc.waypoints.is_empty() {
                        NpcState::Idle
                    } else {
                        NpcState::Patrol
                    });
                }
            }
        }

        if let Some(state) = next_state {
            npc.state = state;
        }
        if let Some(ev) = pending_event {
            game_events.write(GameEvent::Trigger(ev));
        }

        // ── Movement & facing ──────────────────────────────────────────────────
        match npc.state {
            // Standing still states — just face the player if visible.
            NpcState::Idle | NpcState::Interact | NpcState::Alerted => {
                if let Some((_, player_pos, _)) = visible {
                    face_toward(&mut transform, npc_pos, player_pos);
                }
                velocity.linvel.x *= 0.8;
                velocity.linvel.z *= 0.8;
            }

            NpcState::Patrol => {
                if npc.waypoints.is_empty() {
                    velocity.linvel.x *= 0.8;
                    velocity.linvel.z *= 0.8;
                } else {
                    let wp = npc.waypoints[npc.current_waypoint];
                    // Advance to next waypoint when close enough.
                    if npc_pos.distance(wp) < 0.5 {
                        npc.current_waypoint =
                            (npc.current_waypoint + 1) % npc.waypoints.len();
                    }
                    let target_wp = npc.waypoints[npc.current_waypoint];
                    let dir = (target_wp - npc_pos).with_y(0.0);
                    apply_movement(&mut velocity, &mut transform, dir, npc.patrol_speed);
                }
            }

            NpcState::Chase => {
                let move_dir = match npc.on_player_near {
                    // Flee: run in the opposite direction.
                    NpcOnPlayerNear::Flee => visible
                        .map(|(_, p, _)| (npc_pos - p).with_y(0.0))
                        .unwrap_or(Vec3::ZERO),

                    // Chase/Interact: move toward the last known player position.
                    _ => npc.target
                        .and_then(|t| players.iter().find(|(e, _)| *e == t))
                        .map(|(_, p)| (*p - npc_pos).with_y(0.0))
                        .unwrap_or(Vec3::ZERO),
                };
                apply_movement(&mut velocity, &mut transform, move_dir, npc.chase_speed);
            }

            NpcState::Return => {
                let dir = (npc.origin - npc_pos).with_y(0.0);
                apply_movement(&mut velocity, &mut transform, dir, npc.patrol_speed);
            }
        }
    }
}

// ── Small helpers ─────────────────────────────────────────────────────────────

/// Set linear velocity toward `dir` and rotate the entity to face that direction.
/// Applies drag when `dir` is near-zero.
fn apply_movement(velocity: &mut Velocity, transform: &mut Transform, dir: Vec3, speed: f32) {
    if dir.length_squared() > 0.01 {
        let norm = dir.normalize();
        velocity.linvel.x = norm.x * speed;
        velocity.linvel.z = norm.z * speed;
        transform.look_to(norm, Vec3::Y);
    } else {
        velocity.linvel.x *= 0.8;
        velocity.linvel.z *= 0.8;
    }
}

/// Rotate `transform` so it faces `target_pos` (XZ plane only).
fn face_toward(transform: &mut Transform, self_pos: Vec3, target_pos: Vec3) {
    let dir = (target_pos - self_pos).with_y(0.0);
    if dir.length_squared() > 0.01 {
        transform.look_to(dir.normalize(), Vec3::Y);
    }
}

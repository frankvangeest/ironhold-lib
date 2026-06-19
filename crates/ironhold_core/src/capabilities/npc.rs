use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::schema::catalog::{NpcFaction, NpcOnPlayerNear};
use crate::schema::stats::StatMap;
use crate::capabilities::player::CharacterController;
use crate::capabilities::animation_resolver::LocomotionState;
use crate::runtime::messages::*;
use std::collections::{HashMap, HashSet};

/// Populated by `npc_hit_relay_system` (Update, after `action_executor_system`) when an
/// `entity.attacked:*` event fires. Maps NPC id → attacker world position.
/// Drained by `npc_behavior_system` (FixedUpdate) via `std::mem::take`.
#[derive(Resource, Default)]
pub struct NpcHitQueue(pub HashMap<String, Vec3>);

/// Populated by `npc_hit_relay_system` when `stat.{id}.health.depleted` fires.
/// Drained by `npc_behavior_system` to transition NPCs into `NpcState::Dead`.
#[derive(Resource, Default)]
pub struct NpcDeadQueue(pub HashSet<String>);

/// Populated by `npc_hit_relay_system` when `npc.revive:{id}` fires (emitted by
/// behavior "alive" entry_actions). Drained by `npc_behavior_system` to reset Dead NPCs.
#[derive(Resource, Default)]
pub struct NpcReviveQueue(pub HashSet<String>);

// ── Runtime state enum ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum NpcState {
    /// Standing still; no waypoints configured.
    Idle,
    /// Walking between patrol waypoints.
    Patrol,
    /// Player spotted — brief pause before acting.
    Alerted,
    /// Moving toward (Chase/Interact) or away from (Flee) a visible player.
    Chase,
    /// Walking toward the attacker's last-known position to get them in visual range.
    /// Transitions to Alerted if detection succeeds, or Return on timeout/arrival.
    Investigating,
    /// Player escaped; walking back to the patrol origin.
    Return,
    /// Within `approach_distance` of a friendly player.
    Interact,
    /// Health depleted — entity stops moving and ignores all state transitions until revived.
    Dead,
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
    /// Eye height above origin for LOS ray casts (metres). From `NpcDef.eye_height`.
    pub eye_height: f32,
    /// Seconds to pause in Alerted before acting. From `NpcDef.alerted_duration`.
    pub alerted_duration: f32,
    /// Velocity decay multiplier when not actively moving. From `NpcDef.drag`.
    pub drag: f32,
    /// Metres from a waypoint to advance to the next. From `NpcDef.waypoint_reach_radius`.
    pub waypoint_reach_radius: f32,
    /// Multiplier on `approach_distance` defining the leave-interact threshold.
    pub interact_leave_factor: f32,
    /// Metres from spawn origin at which Return state ends. From `NpcDef.home_arrival_radius`.
    pub home_arrival_radius: f32,
    /// Seconds to walk toward the last-known attacker position before giving up.
    /// Resets on each subsequent hit (kiting). From `NpcDef.investigate_timeout_secs`.
    pub investigate_timeout_secs: f32,
    /// Seconds to idle at each patrol waypoint. 0.0 = advance immediately.
    pub waypoint_wait_secs: f32,
    /// Countdown timer for waypoint idle pause. Positive = waiting; 0 = not waiting.
    pub waypoint_wait_timer: f32,
    /// Last position the attacker was known to occupy; set on hit, cleared on Return.
    pub last_known_attacker_pos: Option<Vec3>,
    /// Seconds spent in Investigating state; reset on each new hit.
    pub investigate_timer: f32,
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
    eye_height: f32,
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
                let eye = npc_pos + Vec3::Y * eye_height;
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

// ── Hit relay (Update) ────────────────────────────────────────────────────────

/// Runs in Update, after `action_executor_system`.
/// Reads GameEvent messages written in the same Update tick and populates the three NPC queues
/// so `npc_behavior_system` (FixedUpdate) can react next tick.
pub fn npc_hit_relay_system(
    mut reader: MessageReader<GameEvent>,
    player_query: Query<&GlobalTransform, With<CharacterController>>,
    mut hit_queue: ResMut<NpcHitQueue>,
    mut dead_queue: ResMut<NpcDeadQueue>,
    mut revive_queue: ResMut<NpcReviveQueue>,
) {
    let attacker_pos = player_query.iter().next().map(|gt| gt.translation());
    for GameEvent::Trigger(name) in reader.read() {
        if let Some(id) = name.strip_prefix("entity.attacked:") {
            if let Some(pos) = attacker_pos {
                hit_queue.0.insert(id.to_string(), pos);
            }
        } else if let Some(id) = name.strip_prefix("npc.dead:") {
            dead_queue.0.insert(id.to_string());
        } else if let Some(rest) = name.strip_prefix("stat.") {
            if let Some(id) = rest.strip_suffix(".health.depleted") {
                dead_queue.0.insert(id.to_string());
            }
        } else if let Some(id) = name.strip_prefix("npc.revive:") {
            revive_queue.0.insert(id.to_string());
        }
    }
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
        Option<&mut LocomotionState>,
        Option<&Visibility>,
        Option<&StatMap>,
    )>,
    player_query: Query<(Entity, &GlobalTransform), With<CharacterController>>,
    rapier_context: Option<ReadRapierContext>,
    mut hit_queue: ResMut<NpcHitQueue>,
    mut dead_queue: ResMut<NpcDeadQueue>,
    mut revive_queue: ResMut<NpcReviveQueue>,
    mut game_events: MessageWriter<GameEvent>,
) {
    let rapier = rapier_context.as_ref().and_then(|rc| rc.single().ok());
    let dt = time.delta_secs();

    // Drain queues populated by npc_hit_relay_system (Update) in the previous frame.
    let hit_map: HashMap<String, Vec3> = std::mem::take(&mut hit_queue.0);
    let dead_set: HashSet<String> = std::mem::take(&mut dead_queue.0);
    let revive_set: HashSet<String> = std::mem::take(&mut revive_queue.0);

    // Snapshot all player positions once per tick.
    let players: Vec<(Entity, Vec3)> = player_query
        .iter()
        .map(|(e, gt)| (e, gt.translation()))
        .collect();

    for (npc_entity, mut npc, mut transform, global_tf, mut velocity, loco_opt, visibility, stat_map_opt) in &mut npc_query {
        // Skip hidden entities (dead/despawned).
        if visibility.is_some_and(|v| *v == Visibility::Hidden) {
            velocity.linvel.x = 0.0;
            velocity.linvel.z = 0.0;
            continue;
        }

        // Direct health check — catches death in the same FixedUpdate tick that follows the
        // Update where ModifyStat ran, without waiting for the relay queue's extra hop.
        if stat_map_opt
            .and_then(|sm| sm.0.get("health"))
            .is_some_and(|s| s.current <= 0.0)
            && !matches!(npc.state, NpcState::Dead)
        {
            npc.state = NpcState::Dead;
        }

        // Belt-and-suspenders: relay queue also catches death (e.g. from npc.dead:* events).
        if dead_set.contains(&npc.npc_id) {
            npc.state = NpcState::Dead;
        }
        if revive_set.contains(&npc.npc_id) && matches!(npc.state, NpcState::Dead) {
            npc.state = if npc.waypoints.is_empty() { NpcState::Idle } else { NpcState::Patrol };
            npc.waypoint_wait_timer = 0.0;
        }

        // Dead NPCs halt immediately — the behavior file handles the visual (hide + respawn).
        if matches!(npc.state, NpcState::Dead) {
            velocity.linvel.x = 0.0;
            velocity.linvel.z = 0.0;
            if let Some(mut loco) = loco_opt {
                if loco.moving { loco.moving = false; }
                if loco.running { loco.running = false; }
            }
            continue;
        }

        let npc_pos = global_tf.translation();
        let npc_forward = Vec3::from(transform.forward());

        let visible = find_nearest_visible_player(
            npc_entity, npc_pos, npc_forward,
            npc.fov_cos, npc.requires_los, npc.eye_height,
            &players, rapier.as_ref(),
        );

        let dist_opt    = visible.map(|(_, _, d)| d);
        let in_detect   = dist_opt.map(|d| d <= npc.detection_radius).unwrap_or(false);
        // Chase is visibility-only: player must be seen and within chase_radius.
        let in_chase    = dist_opt.map(|d| d <= npc.chase_radius).unwrap_or(false);
        let in_approach = dist_opt.map(|d| d <= npc.approach_distance).unwrap_or(false);

        // Pre-match: resolve hit and aggro eligibility for all states.
        let hit_pos: Option<Vec3> = hit_map.get(npc.npc_id.as_str()).copied();
        let can_aggro = matches!(npc.on_player_near, NpcOnPlayerNear::Chase | NpcOnPlayerNear::Interact);

        // ── State machine ──────────────────────────────────────────────────────
        let mut next_state: Option<NpcState> = None;
        let mut pending_event: Option<String> = None;

        match npc.state {
            NpcState::Idle | NpcState::Patrol => {
                if in_detect {
                    if let Some((e, _, _)) = visible { npc.target = Some(e); }
                    npc.state_timer = 0.0;
                    next_state = Some(NpcState::Alerted);
                } else if let Some(pos) = hit_pos.filter(|_| can_aggro) {
                    // Hit from outside detection radius — walk toward attacker to get visual.
                    npc.last_known_attacker_pos = Some(pos);
                    npc.investigate_timer = 0.0;
                    pending_event = Some(format!("npc.investigating:{}", npc.npc_id));
                    next_state = Some(NpcState::Investigating);
                }
            }

            NpcState::Alerted => {
                npc.state_timer += dt;
                if npc.state_timer >= npc.alerted_duration {
                    pending_event = Some(format!("npc.player_spotted:{}", npc.npc_id));
                    next_state = Some(match npc.on_player_near {
                        NpcOnPlayerNear::Alert => {
                            if npc.waypoints.is_empty() { NpcState::Idle } else { NpcState::Patrol }
                        }
                        _ => NpcState::Chase,
                    });
                }
            }

            NpcState::Chase => {
                // Refresh target while player is visible.
                if let Some((e, _, _)) = visible { npc.target = Some(e); }

                let flee = matches!(npc.on_player_near, NpcOnPlayerNear::Flee);
                if !flee && in_approach {
                    pending_event = Some(format!("npc.player_reached:{}", npc.npc_id));
                    npc.state_timer = 0.0;
                    next_state = Some(NpcState::Interact);
                } else if !in_chase {
                    // Player left visual range — investigate their last known position
                    // rather than snapping straight home.
                    let last_pos = visible.map(|(_, p, _)| p)
                        .or_else(|| npc.target
                            .and_then(|t| players.iter().find(|(e, _)| *e == t))
                            .map(|(_, p)| *p));
                    if let Some(pos) = last_pos.filter(|_| can_aggro) {
                        npc.last_known_attacker_pos = Some(pos);
                        npc.investigate_timer = 0.0;
                        pending_event = Some(format!("npc.investigating:{}", npc.npc_id));
                        next_state = Some(NpcState::Investigating);
                    } else {
                        pending_event = Some(format!("npc.player_lost:{}", npc.npc_id));
                        next_state = Some(NpcState::Return);
                    }
                } else if let Some(pos) = hit_pos {
                    // Fresh hit while chasing — keep last-known position current.
                    npc.last_known_attacker_pos = Some(pos);
                }
            }

            NpcState::Investigating => {
                npc.investigate_timer += dt;

                if in_detect {
                    // Spotted the player — escalate to Alerted → Chase.
                    if let Some((e, _, _)) = visible { npc.target = Some(e); }
                    npc.state_timer = 0.0;
                    next_state = Some(NpcState::Alerted);
                } else if let Some(pos) = hit_pos {
                    // Another hit: update direction, reset timer (kiting).
                    npc.last_known_attacker_pos = Some(pos);
                    npc.investigate_timer = 0.0;
                } else if npc.investigate_timer >= npc.investigate_timeout_secs {
                    // Timeout — gave up without finding the player.
                    pending_event = Some(format!("npc.investigation_failed:{}", npc.npc_id));
                    next_state = Some(NpcState::Return);
                } else if npc.last_known_attacker_pos
                    .is_some_and(|dest| npc_pos.distance(dest) < npc.waypoint_reach_radius)
                {
                    // Reached destination without spotting player.
                    pending_event = Some(format!("npc.investigation_failed:{}", npc.npc_id));
                    next_state = Some(NpcState::Return);
                }
            }

            NpcState::Interact => {
                if dist_opt.map(|d| d > npc.approach_distance * npc.interact_leave_factor).unwrap_or(true) {
                    pending_event = Some(format!("npc.player_lost:{}", npc.npc_id));
                    next_state = Some(if npc.waypoints.is_empty() {
                        NpcState::Idle
                    } else {
                        NpcState::Patrol
                    });
                }
            }

            NpcState::Return => {
                if in_detect {
                    if let Some((e, _, _)) = visible { npc.target = Some(e); }
                    npc.state_timer = 0.0;
                    next_state = Some(NpcState::Alerted);
                } else if let Some(pos) = hit_pos.filter(|_| can_aggro) {
                    // Hit while returning — investigate again.
                    npc.last_known_attacker_pos = Some(pos);
                    npc.investigate_timer = 0.0;
                    pending_event = Some(format!("npc.investigating:{}", npc.npc_id));
                    next_state = Some(NpcState::Investigating);
                } else if npc_pos.distance(npc.origin) < npc.home_arrival_radius {
                    next_state = Some(if npc.waypoints.is_empty() {
                        NpcState::Idle
                    } else {
                        NpcState::Patrol
                    });
                }
            }

            // Dead is handled above via the `continue` — this arm satisfies exhaustive matching.
            NpcState::Dead => {}
        }

        if let Some(state) = next_state {
            npc.state = state;
        }
        if let Some(ev) = pending_event {
            game_events.write(GameEvent::Trigger(ev));
        }

        // ── Movement & facing ──────────────────────────────────────────────────
        let drag = npc.drag;
        match npc.state {
            NpcState::Idle | NpcState::Interact | NpcState::Alerted => {
                if let Some((_, player_pos, _)) = visible {
                    face_toward(&mut transform, npc_pos, player_pos);
                }
                velocity.linvel.x *= drag;
                velocity.linvel.z *= drag;
            }

            NpcState::Patrol => {
                if npc.waypoints.is_empty() {
                    velocity.linvel.x *= drag;
                    velocity.linvel.z *= drag;
                } else {
                    let wp = npc.waypoints[npc.current_waypoint];
                    if npc.waypoint_wait_timer > 0.0 {
                        // Counting down idle pause at waypoint.
                        npc.waypoint_wait_timer -= dt;
                        if npc.waypoint_wait_timer <= 0.0 {
                            npc.waypoint_wait_timer = 0.0;
                            npc.current_waypoint =
                                (npc.current_waypoint + 1) % npc.waypoints.len();
                        }
                        velocity.linvel.x *= drag;
                        velocity.linvel.z *= drag;
                    } else if npc_pos.distance(wp) < npc.waypoint_reach_radius {
                        if npc.waypoint_wait_secs > 0.0 {
                            npc.waypoint_wait_timer = npc.waypoint_wait_secs;
                            velocity.linvel.x *= drag;
                            velocity.linvel.z *= drag;
                        } else {
                            npc.current_waypoint =
                                (npc.current_waypoint + 1) % npc.waypoints.len();
                            let dir = (npc.waypoints[npc.current_waypoint] - npc_pos).with_y(0.0);
                            apply_movement(&mut velocity, &mut transform, dir, npc.patrol_speed, drag);
                        }
                    } else {
                        let dir = (wp - npc_pos).with_y(0.0);
                        apply_movement(&mut velocity, &mut transform, dir, npc.patrol_speed, drag);
                    }
                }
            }

            NpcState::Chase => {
                let move_dir = match npc.on_player_near {
                    NpcOnPlayerNear::Flee => visible
                        .map(|(_, p, _)| (npc_pos - p).with_y(0.0))
                        .unwrap_or(Vec3::ZERO),
                    _ => npc.target
                        .and_then(|t| players.iter().find(|(e, _)| *e == t))
                        .map(|(_, p)| (*p - npc_pos).with_y(0.0))
                        .unwrap_or(Vec3::ZERO),
                };
                apply_movement(&mut velocity, &mut transform, move_dir, npc.chase_speed, drag);
            }

            NpcState::Investigating => {
                let dir = npc.last_known_attacker_pos
                    .map(|dest| (dest - npc_pos).with_y(0.0))
                    .unwrap_or(Vec3::ZERO);
                apply_movement(&mut velocity, &mut transform, dir, npc.patrol_speed, drag);
            }

            NpcState::Return => {
                let dir = (npc.origin - npc_pos).with_y(0.0);
                apply_movement(&mut velocity, &mut transform, dir, npc.patrol_speed, drag);
            }

            // Unreachable: Dead NPCs `continue` before reaching this match.
            NpcState::Dead => {}
        }

        // Update LocomotionState for GLB NPC entities that have an animation policy.
        if let Some(mut loco) = loco_opt {
            let moving = velocity.linvel.x.powi(2) + velocity.linvel.z.powi(2) > 0.1;
            let running = matches!(npc.state, NpcState::Chase) && moving;
            if loco.moving != moving { loco.moving = moving; }
            if loco.running != running { loco.running = running; }
            if !loco.is_grounded { loco.is_grounded = true; }
        }
    }
}

// ── Small helpers ─────────────────────────────────────────────────────────────

/// Set linear velocity toward `dir` and rotate the entity to face that direction.
/// Applies drag when `dir` is near-zero.
fn apply_movement(velocity: &mut Velocity, transform: &mut Transform, dir: Vec3, speed: f32, drag: f32) {
    if dir.length_squared() > 0.01 {
        let norm = dir.normalize();
        velocity.linvel.x = norm.x * speed;
        velocity.linvel.z = norm.z * speed;
        transform.look_to(norm, Vec3::Y);
    } else {
        velocity.linvel.x *= drag;
        velocity.linvel.z *= drag;
    }
}

/// Rotate `transform` so it faces `target_pos` (XZ plane only).
fn face_toward(transform: &mut Transform, self_pos: Vec3, target_pos: Vec3) {
    let dir = (target_pos - self_pos).with_y(0.0);
    if dir.length_squared() > 0.01 {
        transform.look_to(dir.normalize(), Vec3::Y);
    }
}

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::capabilities::player::CharacterController;
use crate::runtime::messages::*;
use crate::runtime::scene_manager::SpawnId;

/// Marker component for trigger-zone entities.
///
/// The entity must also have a Rapier `Collider + Sensor + ActiveEvents::COLLISION_EVENTS`
/// (added automatically by the spawner when `trigger_zone` is set on the `PrefabDef`).
///
/// On player enter/exit the system emits:
///   `GameEvent::Trigger("entity.entered:{spawn_id}")`
///   `GameEvent::Trigger("entity.exited:{spawn_id}")`
///
/// The response (PlaySound, IncrementVariable, Despawn, etc.) is wired entirely in RON rules
/// or entity behavior files — no Rust changes needed.
#[derive(Component)]
pub struct TriggerZone;

/// Reads Rapier `CollisionEvent` pairs and emits enter/exit game events for any
/// `TriggerZone` entity overlapped by the player. Runs in `FixedUpdate` alongside
/// other physics-driven capability systems.
pub fn trigger_zone_system(
    mut collision_events: MessageReader<CollisionEvent>,
    players: Query<(), With<CharacterController>>,
    zones: Query<&SpawnId, With<TriggerZone>>,
    mut game_events: MessageWriter<GameEvent>,
) {
    for event in collision_events.read() {
        let (e1, e2, started) = match event {
            CollisionEvent::Started(a, b, _) => (*a, *b, true),
            CollisionEvent::Stopped(a, b, _) => (*a, *b, false),
        };

        let zone_entity = if players.contains(e1) && zones.contains(e2) {
            e2
        } else if players.contains(e2) && zones.contains(e1) {
            e1
        } else {
            continue;
        };

        if let Ok(spawn_id) = zones.get(zone_entity) {
            if started {
                info!("TriggerZone entered: {}", spawn_id.0);
                game_events.write(GameEvent::Trigger(
                    format!("entity.entered:{}", spawn_id.0),
                ));
            } else {
                info!("TriggerZone exited: {}", spawn_id.0);
                game_events.write(GameEvent::Trigger(
                    format!("entity.exited:{}", spawn_id.0),
                ));
            }
        }
    }
}

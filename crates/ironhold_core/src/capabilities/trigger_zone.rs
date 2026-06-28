use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::capabilities::player::CharacterController;
use crate::runtime::messages::*;

/// Marker component for trigger-zone entities.
///
/// The sensor lives on a dedicated child entity (spawned automatically when
/// `trigger_zone` is set on a `PrefabDef`) so that its `Collider::ball` does not
/// share the Bevy component slot with any physical `Collider::compound` on the
/// parent. The child carries `TriggerZoneId` instead of `SpawnId` to avoid
/// conflicting with the parent's registry entry.
///
/// On player enter/exit the system emits:
///   `GameEvent::Trigger("entity.entered:{id}")`
///   `GameEvent::Trigger("entity.exited:{id}")`
///
/// The response (PlaySound, IncrementVariable, Despawn, etc.) is wired entirely in RON rules
/// or entity behavior files — no Rust changes needed.
#[derive(Component)]
pub struct TriggerZone;

/// Carries the owning entity's spawn id on the trigger-zone sensor child entity.
/// Kept separate from `SpawnId` so the sensor child is not reachable via
/// `SpawnRegistry` or the `Action::Despawn` entity iterator.
#[derive(Component)]
pub struct TriggerZoneId(pub String);

/// Reads Rapier `CollisionEvent` pairs and emits enter/exit game events for any
/// `TriggerZone` entity overlapped by the player. Runs in `FixedUpdate` alongside
/// other physics-driven capability systems.
pub fn trigger_zone_system(
    mut collision_events: MessageReader<CollisionEvent>,
    players: Query<(), With<CharacterController>>,
    zones: Query<&TriggerZoneId, With<TriggerZone>>,
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

        if let Ok(zone_id) = zones.get(zone_entity) {
            if started {
                info!("TriggerZone entered: {}", zone_id.0);
                game_events.write(GameEvent::Trigger(
                    format!("entity.entered:{}", zone_id.0),
                ));
            } else {
                info!("TriggerZone exited: {}", zone_id.0);
                game_events.write(GameEvent::Trigger(
                    format!("entity.exited:{}", zone_id.0),
                ));
            }
        }
    }
}

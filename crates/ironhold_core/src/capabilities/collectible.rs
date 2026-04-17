use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::capabilities::player::CharacterController;
use crate::runtime::messages::GameEvent;
use crate::runtime::scene_manager::SpawnId;

/// Marker component placed on any sensor entity that should be removed when the player
/// walks through it. The entity must also have `Collider + Sensor + ActiveEvents::COLLISION_EVENTS`
/// so Rapier generates `CollisionEvent`s on overlap.
///
/// On overlap, `collectible_system` emits:
///   `GameEvent::Trigger("entity.collected:{spawn_id}")`
///
/// This fires into the normal Message → Interpreter → Action → Executor pipeline.
/// What actually happens on collection (Despawn, PlaySound, AddScore, etc.) is defined
/// entirely in `state_machine.ron` — no recompile needed to change collectible behaviour.
#[derive(Component)]
pub struct Collectable;

/// Reads Rapier `CollisionEvent::Started` pairs and fires a `UiEvent` trigger for any
/// `Collectable` entity touched by the player. The trigger name follows the convention
/// `"entity.collected:{spawn_id}"` so rules in `state_machine.ron` can match it.
///
/// Runs in `FixedUpdate` alongside the rest of the physics pipeline.
pub fn collectible_system(
    mut collision_events: MessageReader<CollisionEvent>,
    players: Query<(), With<CharacterController>>,
    collectibles: Query<&SpawnId, With<Collectable>>,
    mut game_events: MessageWriter<GameEvent>,
) {
    for event in collision_events.read() {
        let CollisionEvent::Started(e1, e2, _flags) = event else { continue };

        let collectible_entity = if players.contains(*e1) && collectibles.contains(*e2) {
            *e2
        } else if players.contains(*e2) && collectibles.contains(*e1) {
            *e1
        } else {
            continue;
        };

        if let Ok(spawn_id) = collectibles.get(collectible_entity) {
            info!("Collected: {}", spawn_id.0);
            game_events.write(GameEvent::Trigger(format!("entity.collected:{}", spawn_id.0)));
        }
    }
}

use bevy::prelude::*;

use crate::runtime::{ActionQueue, SpawnId};
use crate::schema::Action;

/// Despawns the entity it's attached to once `remaining_secs` counts down to zero. Set via
/// `Action::SetDespawnTimer`.
///
/// Unlike `EmitEventAfterDelay` + a `Despawn` action reacting to a global string event, this
/// timer lives directly on the entity it targets, so it can never leak across entity generations
/// that happen to share a spawn id — the failure mode this was originally built to close:
/// `monster_corpse_loot.md`'s corpses used a fixed, deliberately-reused `"{self}_corpse"` id
/// before `corpse_new_id_retrofit` (2026-08-31) gave each corpse its own unique
/// `"{self}_corpse_{new_id}"` id instead (see `docs/30_runtime_events_and_logic.md`'s "Lootable
/// corpse (loot-on-death)" section). Under that older design, a global delayed event had no
/// owner: `corpse.decay:zombie_01_corpse` armed by one corpse generation would still fire and
/// match whichever entity currently held that id, including an unrelated *later* corpse spawned
/// under the same reused id — found by debug-detective review to be a real, escalating bug over
/// extended play (each kill cycle left one dangling timer; after enough cycles a slot's corpses
/// were despawned within seconds of spawning, permanently, well before their intended decay).
/// A component-based timer can't have this problem by construction: despawning the entity through
/// any other means removes this component with it, and a still-ticking timer can only ever affect
/// the one entity it's actually attached to — this remains the right default for any per-entity
/// decay timer even now that ids are unique, since it needs no global event-name bookkeeping at
/// all to stay safe.
#[derive(Component)]
pub struct DespawnTimer {
    pub remaining_secs: f32,
}

/// Ticks every `DespawnTimer` down by real elapsed time and, once it reaches zero, queues an
/// `Action::Despawn` for that entity rather than despawning it directly with `Commands`.
///
/// This indirection matters: `Action::Despawn`'s own handler in `action_executor.rs` also removes
/// the entity from `SpawnRegistry` and closes the container panel if that entity was the currently
/// open one (see `monster_corpse_loot.md`'s Finding 2/3 fixes). A direct `commands.despawn()` here
/// would bypass both — confirmed by a real playtest report: a corpse's ambient decay firing while
/// its loot panel was open left the panel and target UI stuck, because the registry entry (and
/// hence every system that looks the id up through it, including the container-close check and
/// `target_auto_clear_system`) was never cleared. Routing through the same `Action::Despawn` path
/// every other despawn uses keeps that teardown logic in one place.
///
/// The `DespawnTimer` component is removed the instant it expires (not left to tick further),
/// so a still-armed timer can't queue a second `Despawn` for the same entity while the first is
/// waiting to execute.
pub fn despawn_timer_system(
    mut commands: Commands,
    mut action_queue: ResMut<ActionQueue>,
    time: Res<Time>,
    mut query: Query<(Entity, &mut DespawnTimer, Option<&SpawnId>)>,
) {
    let dt = time.delta_secs();
    for (entity, mut timer, spawn_id) in query.iter_mut() {
        timer.remaining_secs -= dt;
        if timer.remaining_secs <= 0.0 {
            commands.entity(entity).remove::<DespawnTimer>();
            match spawn_id {
                Some(id) => action_queue.push(Action::Despawn(id.0.clone())),
                // No SpawnId to route through Action::Despawn's registry/UI-teardown logic —
                // shouldn't happen in practice (SetDespawnTimer only resolves entities that
                // already have one), but fall back to a direct despawn rather than leaking the
                // entity forever.
                None => {
                    commands.entity(entity).try_despawn();
                }
            }
        }
    }
}

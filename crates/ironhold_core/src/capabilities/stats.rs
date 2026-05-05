use bevy::prelude::*;
use crate::runtime::messages::GameEvent;
use crate::schema::stats::LoadedStats;

/// Ticks the regen cooldown and applies regen_rate each frame.
/// Runs before the interpreter chain so regen-triggered threshold crossings
/// are detected by `stat_threshold_system` in the same frame.
pub fn stat_regen_system(
    mut loaded_stats: ResMut<LoadedStats>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for stat in loaded_stats.0.values_mut() {
        if stat.def.regen_rate == 0.0 { continue; }
        if stat.regen_cooldown > 0.0 {
            stat.regen_cooldown = (stat.regen_cooldown - dt).max(0.0);
        } else if stat.current < stat.def.max {
            stat.current = (stat.current + stat.def.regen_rate * dt).min(stat.def.max);
        }
    }
}

/// Checks each stat's thresholds after mutations and emits a `GameEvent::Trigger`
/// on the frame a condition transitions from false to true.
/// Edge-triggered: does not re-fire every frame while the condition remains true.
/// Threshold events are processed by the interpreters the following frame.
pub fn stat_threshold_system(
    mut loaded_stats: ResMut<LoadedStats>,
    mut game_events: MessageWriter<GameEvent>,
) {
    for (stat_key, stat) in loaded_stats.0.iter_mut() {
        let max = stat.def.max;
        let current = stat.current;
        for (i, threshold) in stat.def.thresholds.iter().enumerate() {
            let is_met = threshold.when.is_met(current, max);
            let was_met = stat.prev_threshold_states.get(i).copied().unwrap_or(false);
            if is_met && !was_met {
                info!(
                    "stat threshold crossed: stat=\"{}\" -> emitting \"{}\"",
                    stat_key, threshold.emit
                );
                game_events.write(GameEvent::Trigger(threshold.emit.clone()));
            }
            if let Some(prev) = stat.prev_threshold_states.get_mut(i) {
                *prev = is_met;
            }
        }
    }
}

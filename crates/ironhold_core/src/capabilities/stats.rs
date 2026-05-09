use bevy::prelude::*;
use crate::runtime::messages::GameEvent;
use crate::schema::stats::{LiveStat, LoadedStats, StatMap};

/// Ticks the regen cooldown and applies regen_rate each frame for both global and instance stats.
/// Runs before the interpreter chain so regen-triggered threshold crossings
/// are detected by `stat_threshold_system` in the same frame.
pub fn stat_regen_system(
    mut loaded_stats: ResMut<LoadedStats>,
    mut stat_map_query: Query<&mut StatMap>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for stat in loaded_stats.0.values_mut() {
        tick_regen(stat, dt);
    }
    for mut stat_map in stat_map_query.iter_mut() {
        for stat in stat_map.0.values_mut() {
            tick_regen(stat, dt);
        }
    }
}

fn tick_regen(stat: &mut LiveStat, dt: f32) {
    if stat.def.regen_rate == 0.0 { return; }
    if stat.regen_cooldown > 0.0 {
        stat.regen_cooldown = (stat.regen_cooldown - dt).max(0.0);
    } else if stat.current < stat.def.max {
        stat.current = (stat.current + stat.def.regen_rate * dt).min(stat.def.max);
    }
}

/// Checks each stat's thresholds after mutations and emits a `GameEvent::Trigger`
/// on the frame a condition transitions from false to true.
/// Edge-triggered: does not re-fire every frame while the condition remains true.
/// Covers both global `LoadedStats` and per-entity `StatMap` components.
pub fn stat_threshold_system(
    mut loaded_stats: ResMut<LoadedStats>,
    mut stat_map_query: Query<&mut StatMap>,
    mut game_events: MessageWriter<GameEvent>,
) {
    for (stat_key, stat) in loaded_stats.0.iter_mut() {
        fire_threshold_crossings(stat_key, stat, &mut game_events);
    }
    for mut stat_map in stat_map_query.iter_mut() {
        let keys: Vec<String> = stat_map.0.keys().cloned().collect();
        for stat_key in &keys {
            if let Some(stat) = stat_map.0.get_mut(stat_key) {
                fire_threshold_crossings(stat_key, stat, &mut game_events);
            }
        }
    }
}

fn fire_threshold_crossings(
    stat_key: &str,
    stat: &mut LiveStat,
    game_events: &mut MessageWriter<GameEvent>,
) {
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

use bevy::prelude::*;
use crate::runtime::messages::GameEvent;
use crate::schema::stats::{LiveStat, LoadedStats, LoadedModifiers, StatMap};

/// Ticks all active modifier durations and removes expired ones.
/// Emits `stat.modifier.expired:{key}` for each unique modifier key that expired this frame.
/// Must run before `stat_regen_system` so expiry is visible in the same frame.
pub fn stat_modifier_system(
    mut loaded_stats: ResMut<LoadedStats>,
    mut stat_map_query: Query<&mut StatMap>,
    mut game_events: MessageWriter<GameEvent>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (stat_key, stat) in loaded_stats.0.iter_mut() {
        tick_modifiers(stat_key, stat, dt, &mut game_events);
    }
    for mut stat_map in stat_map_query.iter_mut() {
        let keys: Vec<String> = stat_map.0.keys().cloned().collect();
        for stat_key in &keys {
            if let Some(stat) = stat_map.0.get_mut(stat_key) {
                tick_modifiers(stat_key, stat, dt, &mut game_events);
            }
        }
    }
}

fn tick_modifiers(
    stat_key: &str,
    stat: &mut LiveStat,
    dt: f32,
    game_events: &mut MessageWriter<GameEvent>,
) {
    for am in &mut stat.active_modifiers {
        if let Some(rem) = &mut am.remaining_secs {
            *rem -= dt;
        }
    }

    // Collect unique modifier keys that just expired (deduplicated — one event per key per frame)
    let mut expired_keys: Vec<String> = Vec::new();
    for am in &stat.active_modifiers {
        if let Some(rem) = am.remaining_secs {
            if rem <= 0.0 && !expired_keys.contains(&am.key) {
                expired_keys.push(am.key.clone());
            }
        }
    }

    stat.active_modifiers.retain(|am| am.remaining_secs.map_or(true, |r| r > 0.0));

    for key in &expired_keys {
        let event = format!("stat.modifier.expired:{}", key);
        info!("stat modifier expired: stat=\"{}\" modifier=\"{}\" -> emitting \"{}\"", stat_key, key, event);
        game_events.write(GameEvent::Trigger(event));
    }
}

/// Recomputes the `effective` value for every stat from its active modifier stack.
/// Must run after `stat_modifier_system` and `stat_regen_system`, before `stat_threshold_system`.
pub fn stat_effective_value_system(
    mut loaded_stats: ResMut<LoadedStats>,
    mut stat_map_query: Query<&mut StatMap>,
    modifier_defs: Res<LoadedModifiers>,
) {
    for stat in loaded_stats.0.values_mut() {
        stat.effective = stat.compute_effective(&modifier_defs.0);
    }
    for mut stat_map in stat_map_query.iter_mut() {
        for stat in stat_map.0.values_mut() {
            stat.effective = stat.compute_effective(&modifier_defs.0);
        }
    }
}

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
/// Uses `stat.effective` (not `stat.current`) so modifier-driven threshold crossings fire correctly.
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
    let effective = stat.effective;
    for (i, threshold) in stat.def.thresholds.iter().enumerate() {
        let is_met = threshold.when.is_met(effective, max);
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

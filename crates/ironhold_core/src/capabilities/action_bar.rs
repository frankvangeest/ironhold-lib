use bevy::prelude::*;
use std::collections::HashMap;

use crate::schema::actions::Action;
use crate::schema::scene_v2::SlotCost;
use crate::schema::stats::LoadedStats;
use crate::runtime::actions::ActionQueue;
use crate::runtime::messages::GameEvent;
use crate::runtime::scene_manager::message_interpreter::rewrite_target;

// ─── Resources ────────────────────────────────────────────────────────────────

/// Tracks remaining cooldown (secs) and total cooldown per slot key.
/// Entries are removed when remaining reaches 0.
#[derive(Resource, Default)]
pub struct CooldownMap(pub HashMap<String, (f32, f32)>); // (remaining, total)

/// The entity (spawn ID) currently targeted by the player.
/// Populated by the targeting system when it ships. Defaults to `None`.
/// `{target}` substitution in action bar `do_actions` resolves against this value.
#[derive(Resource, Default)]
pub struct CurrentTarget(pub Option<String>);

// ─── Components ───────────────────────────────────────────────────────────────

/// Attached to each slot button entity by the scene loader.
#[derive(Component, Clone)]
pub struct ActionSlotUi {
    pub slot_key: String,
    pub do_actions: Vec<Action>,
    pub cooldown_secs: Option<f32>,
    pub cost: Option<SlotCost>,
}

/// Overlay child that fills from the top to show remaining cooldown.
/// Height is animated by `action_bar_visual_system`.
#[derive(Component)]
pub struct CooldownOverlay {
    pub slot_key: String,
}

// ─── Plugin ───────────────────────────────────────────────────────────────────

pub struct ActionBarPlugin;

fn any_action_slots(slots: Query<&ActionSlotUi>) -> bool {
    !slots.is_empty()
}

impl Plugin for ActionBarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CooldownMap>()
            .init_resource::<CurrentTarget>()
            .add_systems(
                Update,
                (
                    cooldown_tick_system.run_if(|c: Res<CooldownMap>| !c.0.is_empty()),
                    action_bar_input_system.run_if(any_action_slots),
                    action_bar_visual_system.run_if(any_action_slots),
                )
                    .chain(),
            );
    }
}

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Drains all `CooldownMap` entries by the elapsed frame time; removes expired ones.
/// Skips entirely when the map is empty to avoid a pointless `ResMut` DerefMut every frame.
pub fn cooldown_tick_system(time: Res<Time>, mut cooldowns: ResMut<CooldownMap>) {
    if cooldowns.0.is_empty() { return; }
    let dt = time.delta_secs();
    cooldowns.0.retain(|_, (remaining, _total)| {
        *remaining -= dt;
        *remaining > 0.0
    });
}

/// Listens for digit key presses (1–9), finds the matching `ActionSlotUi`, and either:
/// - fires `do_actions` + starts cooldown + deducts cost, or
/// - emits an `action_bar.*` event describing why the slot didn't fire.
pub fn action_bar_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    slots: Query<&ActionSlotUi>,
    mut action_queue: ResMut<ActionQueue>,
    mut game_events: MessageWriter<GameEvent>,
    mut cooldowns: ResMut<CooldownMap>,
    loaded_stats: Option<Res<LoadedStats>>,
    current_target: Res<CurrentTarget>,
) {
    let pressed_key = DIGIT_KEYS.iter().find(|(kc, _)| keys.just_pressed(*kc));
    let Some(&(_, key_str)) = pressed_key else { return };

    let slot = slots.iter().find(|s| s.slot_key == *key_str);
    let Some(slot) = slot else { return };

    // ── Cooldown check ────────────────────────────────────────────────────────
    if cooldowns.0.contains_key(key_str) {
        game_events.write(GameEvent::Trigger(
            format!("action_bar.on_cooldown:{}", key_str),
        ));
        return;
    }

    // ── Cost check ────────────────────────────────────────────────────────────
    if let Some(cost) = &slot.cost {
        let current = loaded_stats
            .as_ref()
            .and_then(|ls| ls.0.get(&cost.stat))
            .map(|s| s.current)
            .unwrap_or(0.0);
        if current < cost.amount {
            game_events.write(GameEvent::Trigger(
                format!("action_bar.insufficient_resource:{}", key_str),
            ));
            return;
        }
    }

    // ── {target} check ────────────────────────────────────────────────────────
    let needs_target = slot.do_actions.iter().any(action_needs_target);
    if needs_target && current_target.0.is_none() {
        game_events.write(GameEvent::Trigger(
            format!("action_bar.no_target:{}", key_str),
        ));
        return;
    }

    // ── Fire ─────────────────────────────────────────────────────────────────
    // Substitute {target} in actions if a target is available.
    let target_id = current_target.0.as_deref().unwrap_or("");
    for action in &slot.do_actions {
        action_queue.push(rewrite_target(action.clone(), target_id));
    }

    // Deduct cost stat.
    if let Some(cost) = &slot.cost {
        action_queue.push(Action::ModifyStat {
            key: cost.stat.clone(),
            delta: -cost.amount,
        });
    }

    // Start cooldown.
    if let Some(cd) = slot.cooldown_secs {
        cooldowns.0.insert(key_str.to_string(), (cd, cd));
    }

    game_events.write(GameEvent::Trigger(
        format!("action_bar.activated:{}", key_str),
    ));
}

/// Updates cooldown overlay alpha each frame.
/// Uses alpha-fade only — no `Node` height writes, so Bevy's UI layout is never invalidated.
/// Alpha encodes both cooldown progress (fades out as cooldown depletes) and blocked state
/// (constant dim when cost stat is insufficient).
pub fn action_bar_visual_system(
    cooldowns: Res<CooldownMap>,
    loaded_stats: Option<Res<LoadedStats>>,
    slots: Query<(&ActionSlotUi, &Children)>,
    mut overlays: Query<(&CooldownOverlay, &mut BackgroundColor)>,
) {
    for (slot, children) in &slots {
        let cd_frac = cooldowns.0.get(&slot.slot_key)
            .map(|(remaining, total)| (remaining / total).clamp(0.0, 1.0))
            .unwrap_or(0.0);

        let cost_ok = slot.cost.as_ref().map(|c| {
            loaded_stats
                .as_ref()
                .and_then(|ls| ls.0.get(&c.stat))
                .map(|s| s.current)
                .unwrap_or(0.0)
                >= c.amount
        }).unwrap_or(true);

        // On cooldown: alpha tracks remaining fraction (bright when just used, fades as it clears).
        // Blocked by cost: constant dim at 0.45.
        // Ready: transparent.
        let target_alpha: f32 = if cd_frac > 0.0 {
            0.25 + cd_frac * 0.45
        } else if !cost_ok {
            0.45
        } else {
            0.0
        };

        for child in children.iter() {
            if let Ok((overlay, mut bg)) = overlays.get_mut(child) {
                if overlay.slot_key != slot.slot_key { continue; }
                if (bg.0.alpha() - target_alpha).abs() > 0.01 {
                    bg.0 = bg.0.with_alpha(target_alpha);
                }
            }
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

const DIGIT_KEYS: &[(KeyCode, &str)] = &[
    (KeyCode::Digit1, "1"),
    (KeyCode::Digit2, "2"),
    (KeyCode::Digit3, "3"),
    (KeyCode::Digit4, "4"),
    (KeyCode::Digit5, "5"),
    (KeyCode::Digit6, "6"),
    (KeyCode::Digit7, "7"),
    (KeyCode::Digit8, "8"),
    (KeyCode::Digit9, "9"),
    // Letter slots — used for utility actions (e.g. "i" = inventory toggle).
    (KeyCode::KeyI, "i"),
];

fn action_needs_target(action: &Action) -> bool {
    match action {
        Action::ModifyStat { key, .. } => key.contains("{target}"),
        Action::SetStat { key, .. } => key.contains("{target}"),
        Action::SpawnEffect { entity, .. } => entity.as_deref() == Some("{target}"),
        Action::ShowDamagePopup { entity, .. } => entity.contains("{target}"),
        Action::ShowFloatingText { entity, .. } => entity.contains("{target}"),
        Action::SetEntityVisible { entity, .. } => entity.contains("{target}"),
        Action::Despawn(s) => s.contains("{target}"),
        Action::EmitEvent(s) => s.contains("{target}"),
        Action::PlayAnimationOn { target, .. } => target.contains("{target}"),
        _ => false,
    }
}

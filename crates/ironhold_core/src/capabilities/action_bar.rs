use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::schema::actions::Action;
use crate::schema::scene_v2::SlotCost;
use crate::schema::stats::LoadedStats;
use crate::runtime::actions::ActionQueue;
use crate::runtime::messages::GameEvent;
use crate::runtime::scene_manager::message_interpreter::rewrite_target;
use crate::runtime::scene_manager::SpawnId;
use crate::capabilities::player::CharacterController;

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

/// Pending slot actions held between `action_bar_input_system` and `flush_pending_intent_system`.
/// Keys are slot keys (e.g., `"1"`, `"i"`).
/// Values are (pre-target-rewritten actions, cooldown_secs).
/// Cooldown is committed by `flush_pending_intent_system` only on the commit path,
/// so a suppressed intent never starts the cooldown timer.
/// Cleared each frame by `flush_pending_intent_system`.
#[derive(Resource, Default)]
pub struct PendingIntentActions(pub HashMap<String, (Vec<Action>, Option<f32>)>);

/// Slot keys whose `intent.slot.*` event was matched by a rule this frame.
/// Written by the interpreter systems; read by `flush_pending_intent_system` to suppress
/// the slot's built-in do_actions when a designer rule took over.
/// Cleared each frame by `flush_pending_intent_system`.
#[derive(Resource, Default)]
pub struct HandledIntentSlots(pub HashSet<String>);

// ─── Components ───────────────────────────────────────────────────────────────

/// Attached to each slot button entity by the scene loader.
#[derive(Component, Clone)]
pub struct ActionSlotUi {
    pub slot_key: String,
    /// `InputMap::parse_key(&slot_key)`, resolved once at scene load. `None` if `slot_key` isn't
    /// a recognised key name (the scene loader already `warn!`s about this at spawn time) — such
    /// a slot never fires, since there's no `KeyCode` to check `just_pressed` against.
    pub resolved_key: Option<KeyCode>,
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
        use crate::runtime::scene_manager::message_interpreter::message_interpreter_system;
        app.init_resource::<CooldownMap>()
            .init_resource::<CurrentTarget>()
            .init_resource::<PendingIntentActions>()
            .init_resource::<HandledIntentSlots>()
            .add_systems(
                Update,
                (
                    cooldown_tick_system.run_if(|c: Res<CooldownMap>| !c.0.is_empty()),
                    action_bar_input_system.run_if(any_action_slots),
                    action_bar_visual_system.run_if(any_action_slots),
                )
                    .chain()
                    .before(message_interpreter_system),
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

/// Listens for any slot's resolved key being pressed, and either:
/// - emits `intent.slot.{key}:{entity}` + stores pending actions for the interpreter pass, or
/// - emits an `action_bar.*` event describing why the slot didn't fire.
///
/// Pending actions are flushed to `ActionQueue` by `flush_pending_intent_system` after all
/// interpreter systems run. If a designer rule matches the intent event, the slot's built-in
/// `do_actions` are suppressed; otherwise they fire unchanged.
///
/// Fire-first semantics: if 2+ slots' keys are `just_pressed` in the same frame, only the first
/// one found (query/spawn iteration order) fires this frame — exactly one slot fired per frame
/// under the old `DIGIT_KEYS` table lookup too (same net behavior for the common case), though the
/// old code tie-broke by fixed key-table order rather than spawn order; the two coincide unless a
/// scene's slots are authored out of digit order and 2+ distinct keys are pressed in one frame, a
/// rare case with no observable difference (one slot fires either way). See
/// `action_bar_custom_hotkeys.md`'s Decisions for why (and its "Relationship to Phase 2" note —
/// the per-player action-bar feature restructures this into a loop over every pressed slot, since
/// two independent players' bars must not drop each other's same-frame presses; that
/// restructuring is out of scope here, where there's exactly one bar).
pub fn action_bar_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    slots: Query<&ActionSlotUi>,
    mut game_events: MessageWriter<GameEvent>,
    cooldowns: Res<CooldownMap>,
    loaded_stats: Option<Res<LoadedStats>>,
    current_target: Res<CurrentTarget>,
    mut pending: ResMut<PendingIntentActions>,
    player_query: Query<&SpawnId, With<CharacterController>>,
) {
    let slot = slots.iter().find(|s| s.resolved_key.is_some_and(|kc| keys.just_pressed(kc)));
    let Some(slot) = slot else { return };
    let key_str = slot.slot_key.as_str();

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
    let target_id = current_target.0.as_deref().unwrap_or("");

    // Emit the intent event. The interpreter checks for a matching rule this frame;
    // if one matches, flush_pending_intent_system suppresses the slot's built-in do_actions.
    let player_id = player_query.single().map(|id| id.0.as_str()).unwrap_or("player");
    game_events.write(GameEvent::Trigger(
        format!("intent.slot.{}:{}", key_str, player_id),
    ));

    // Store pending actions (target-rewritten) + cooldown. Flushed to ActionQueue by
    // flush_pending_intent_system unless a rule handled the intent.
    // Cooldown and `action_bar.activated` are also deferred to flush so that a
    // suppressed intent never starts the cooldown or fires the result event.
    let mut actions: Vec<Action> = slot.do_actions.iter()
        .map(|a| rewrite_target(a.clone(), target_id))
        .collect();
    if let Some(cost) = &slot.cost {
        actions.push(Action::ModifyStat {
            key: cost.stat.clone(),
            delta: -cost.amount,
        });
    }
    pending.0.insert(key_str.to_string(), (actions, slot.cooldown_secs));

    // action_bar.pressed fires immediately (before interpreter) — notification that the key
    // was pressed and passed all gate checks. Use for telemetry or UI feedback that should
    // fire regardless of whether a rule later cancels the intent.
    game_events.write(GameEvent::Trigger(
        format!("action_bar.pressed:{}", key_str),
    ));
}

/// Drains `PendingIntentActions` after all interpreter systems have run.
/// For each pending slot:
/// - If a rule handled its intent (`HandledIntentSlots` contains the key): suppressed — no
///   actions, no cooldown, no `action_bar.activated` event.
/// - Otherwise: pushes actions to `ActionQueue`, commits the cooldown, and emits
///   `action_bar.activated:{key}` so rules can react to the committed ability.
pub fn flush_pending_intent_system(
    mut pending: ResMut<PendingIntentActions>,
    mut handled: ResMut<HandledIntentSlots>,
    mut action_queue: ResMut<ActionQueue>,
    mut cooldowns: ResMut<CooldownMap>,
    mut game_events: MessageWriter<GameEvent>,
) {
    for (slot_key, (actions, cooldown)) in pending.0.drain() {
        if !handled.0.contains(&slot_key) {
            for action in actions {
                action_queue.push(action);
            }
            if let Some(cd) = cooldown {
                cooldowns.0.insert(slot_key.clone(), (cd, cd));
            }
            game_events.write(GameEvent::Trigger(format!("action_bar.activated:{}", slot_key)));
        }
    }
    handled.0.clear();
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

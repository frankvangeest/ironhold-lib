use bevy::prelude::*;
use bevy::input::gamepad::{Gamepad, GamepadButton};
use std::collections::{BTreeMap, HashSet};

use crate::schema::actions::Action;
use crate::schema::scene_v2::SlotCost;
use crate::schema::stats::{LoadedStats, StatMap};
use crate::runtime::actions::ActionQueue;
use crate::runtime::messages::GameEvent;
use crate::runtime::scene_manager::action_substitution::rewrite_target;
use crate::runtime::scene_manager::fsm_interpreter::fsm_interpreter_system;
use crate::runtime::scene_manager::SpawnId;
use crate::capabilities::player::{BoundGamepad, CharacterController, PlayerIndex, PlayerTarget};
use crate::capabilities::targeting::is_primary_player;

// ─── Resources ────────────────────────────────────────────────────────────────

/// Tracks remaining cooldown (secs) and total cooldown per slot key.
/// Entries are removed when remaining reaches 0.
/// A `BTreeMap` (sorted by slot key), not a `HashMap`: std hashers are randomly seeded, and this
/// map is iterated (`cooldown_tick_system`'s `retain`), so a hash map would give a different
/// iteration order on every run / platform (determinism: `planning/investigations/
/// hashmap_iteration_order_audit.md`).
#[derive(Resource, Default)]
pub struct CooldownMap(pub BTreeMap<String, (f32, f32)>); // (remaining, total)

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
///
/// A `BTreeMap`, not a `HashMap`: `flush_pending_intent_system` drains it into `ActionQueue`, so
/// the iteration order IS the push order of every simultaneously-fired slot's actions. Sorted by
/// slot key it is identical on every run and platform (a hash map gave a random order: with a heal
/// `+50` and a sacrifice `-60` fired in one frame the clamped result depended on the hasher seed).
#[derive(Resource, Default)]
pub struct PendingIntentActions(pub BTreeMap<String, (Vec<Action>, Option<f32>)>);

/// Slot keys whose `intent.slot.*` event was matched by a rule this frame.
/// Written by the interpreter systems; read by `flush_pending_intent_system` to suppress
/// the slot's built-in do_actions when a designer rule took over.
/// Cleared each frame by `flush_pending_intent_system`.
#[derive(Resource, Default)]
pub struct HandledIntentSlots(pub HashSet<String>); // det: lookup-only

// ─── Components ───────────────────────────────────────────────────────────────

/// Attached to each slot button entity by the scene loader.
#[derive(Component, Clone)]
pub struct ActionSlotUi {
    pub slot_key: String,
    /// `InputMap::parse_key(&slot_key)`, resolved once at scene load. `None` if `slot_key` isn't
    /// a recognised key name (the scene loader already `warn!`s about this at spawn time) — such
    /// a slot never fires from the keyboard (there's no `KeyCode` to check `just_pressed` against);
    /// clicking the slot still fires it.
    pub resolved_key: Option<KeyCode>,
    /// `InputMap::parse_gamepad_button(&gamepad_key)`, resolved once at scene load. `None` when
    /// `gamepad_key` was omitted (ordinary keyboard-only slot) or unparseable (the scene loader
    /// already `warn!`s about this at spawn time) — such a slot never fires from gamepad.
    pub resolved_gamepad_button: Option<GamepadButton>,
    pub do_actions: Vec<Action>,
    pub cooldown_secs: Option<f32>,
    pub cost: Option<SlotCost>,
    /// Copied from `ActionBarDef.owner_player`. `None` means the default single-shared-bar
    /// behavior (resolves against the primary player); `Some(n)` scopes this slot to whichever
    /// player entity carries `PlayerIndex(n)`.
    pub owner_player: Option<u32>,
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
                    .before(fsm_interpreter_system),
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

/// Listens for every slot's resolved key being pressed this frame, and for each fired slot
/// either:
/// - emits `intent.slot.{key}:{entity}` + stores pending actions for the interpreter pass, or
/// - emits an `action_bar.*` event describing why the slot didn't fire.
///
/// Pending actions are flushed to `ActionQueue` by `flush_pending_intent_system` after all
/// interpreter systems run. If a designer rule matches the intent event, the slot's built-in
/// `do_actions` are suppressed; otherwise they fire unchanged.
///
/// Loops over **every** slot whose resolved key is `just_pressed`, not just the first match —
/// per-player action bars (`ActionBarDef.owner_player`, see
/// `planning/features/per_player_split_screen_targeting.md` Phase 2) mean 2+ independent bars can
/// have both players press their own key in the same frame; the earlier `find`+`return` structure
/// (see `action_bar_custom_hotkeys.md`) would have silently dropped one of them.
///
/// A slot fires on **either** device, but the two resolve differently
/// (`planning/features/gamepad_action_bar_slots.md`). Keyboard is genuinely shared hardware: any
/// slot's `key` fires from the one global `ButtonInput<KeyCode>` regardless of who's "supposed" to
/// press it — pre-existing, unchanged behavior. A gamepad is not shared the same way, so a slot's
/// `gamepad_key` only fires from its **owning player's own** controller (`owner_player` ->
/// that player's `BoundGamepad`) — never any connected pad. The fast-path skip below
/// (no keyboard press AND no `gamepad_key` bound) preserves today's exact perf profile and
/// on-unmatched-owner cooldown-event behavior for every **keyboard-only** slot (no gamepad_key at
/// all — the common case today); a slot that does declare `gamepad_key` resolves its owning
/// player every frame regardless of whether anything was actually pressed, same as any other
/// per-frame per-slot player lookup in this system.
///
/// A slot can also be **clicked**: each slot is a `Button`, so Bevy tracks an `Interaction` on it,
/// and the press edge (`Interaction` changed to `Pressed` since this system last ran — detected with
/// `Ref::is_changed`, so it fires once per press, never while held and never on release) counts as
/// a third fire source alongside keyboard and gamepad
/// (`planning/features/done/action_bar_mouse_click.md`). The mouse is shared hardware like the keyboard,
/// so a click acts for the slot's own `owner_player` regardless of which split-screen viewport the
/// cursor is in. `Interaction` is `Option`al so a bare `ActionSlotUi` (no `Button`, as in tests)
/// still matches the query. Clicks — not key presses — are ignored while the inspector is enabled,
/// since egui and Bevy UI both receive every click with no arbitration (same gate as
/// `button_system`). Click, key and button collapse into
/// one "fired" decision per slot, so simultaneous sources activate it exactly once.
///
/// Each fired slot resolves its **owning player** — `owner_player: Some(n)` matches whichever
/// player entity carries `PlayerIndex(n)`; `None` (or `Some(0)`) matches the primary player
/// (`PlayerIndex(0)` or no `PlayerIndex` at all, same definition `is_primary_player` uses
/// elsewhere). That player's own `PlayerTarget` — not the global `CurrentTarget` resource — drives
/// the `{target}` rewrite, the no-target gate, and the `intent.slot.*:{player_id}` event. For the
/// primary player this is a no-op in practice, since `PlayerTarget` is already kept in lockstep
/// with `CurrentTarget` for the primary player (Phase 1). A slot whose `owner_player` doesn't
/// match any player entity present in the scene never fires (nothing to resolve a target or an
/// acting player id against).
pub fn action_bar_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    slots: Query<(&ActionSlotUi, Option<Ref<Interaction>>)>,
    mut game_events: MessageWriter<GameEvent>,
    cooldowns: Res<CooldownMap>,
    loaded_stats: Option<Res<LoadedStats>>,
    mut pending: ResMut<PendingIntentActions>,
    players: Query<(&SpawnId, &PlayerTarget, Option<&PlayerIndex>, Option<&StatMap>, &CharacterController, Option<&BoundGamepad>)>,
    gamepad_query: Query<&Gamepad>,
    #[cfg(feature = "inspector")]
    inspector_enabled: Option<Res<crate::inspector::InspectorEnabled>>,
) {
    #[cfg(feature = "inspector")]
    let clicks_enabled = !inspector_enabled.is_some_and(|e| e.0);
    #[cfg(not(feature = "inspector"))]
    let clicks_enabled = true;

    let mut events: Vec<(&str, GameEvent)> = Vec::new();

    for (slot, interaction) in slots.iter() {
        // Keyboard and mouse are shared hardware: both are "device-independent" fires, gated
        // before the owning player is resolved (the same ordering the keyboard-only path always
        // had, preserved byte-for-byte). Distinct from the gamepad, which needs its owning
        // player's own controller and is therefore resolved after the player lookup.
        let key_pressed = slot.resolved_key.is_some_and(|kc| keys.just_pressed(kc));
        let click_fired = clicks_enabled
            && interaction.is_some_and(|i| i.is_changed() && *i == Interaction::Pressed);
        let direct_fired = key_pressed || click_fired;
        // Fast path: unchanged perf profile for the common case (no gamepad binding, not pressed).
        if !direct_fired && slot.resolved_gamepad_button.is_none() { continue; }

        let key_str = slot.slot_key.as_str();

        // ── Cooldown check (keyboard / click) ────────────────────────────────────────────
        // Gated before player resolution, exactly as before this feature — preserves the
        // keyboard-only on-unmatched-owner cooldown-event behavior byte-for-byte. A gamepad press
        // can't be checked yet: `gamepad_fired` needs the owning player's own `gamepad_index`,
        // resolved below.
        if direct_fired && cooldowns.0.contains_key(key_str) {
            events.push((key_str, GameEvent::Trigger(
                format!("action_bar.on_cooldown:{}", key_str),
            )));
            continue;
        }

        // ── Resolve the acting player for this slot ─────────────────────────────
        let Some((spawn_id, player_target, _, player_stats, _controller, bound)) = players.iter()
            .find(|(_, _, idx, _, _, _)| owns_slot(slot.owner_player, *idx))
        else { continue };

        let gamepad_fired = slot.resolved_gamepad_button.is_some_and(|btn| {
            bound.and_then(|b| b.0).and_then(|e| gamepad_query.get(e).ok())
                .is_some_and(|gp| gp.just_pressed(btn))
        });
        if !direct_fired && !gamepad_fired { continue; }

        // ── Cooldown check (gamepad-only fire) ───────────────────────────────────
        // Mirrors the keyboard check above for the one case it couldn't cover: a gamepad press
        // with no keyboard press or click this frame. `direct_fired` presses were already handled (and
        // returned) above, so this can't double-emit.
        if !direct_fired && cooldowns.0.contains_key(key_str) {
            events.push((key_str, GameEvent::Trigger(
                format!("action_bar.on_cooldown:{}", key_str),
            )));
            continue;
        }

        // ── Cost check ────────────────────────────────────────────────────────
        // Resolved once (own StatMap vs. global LoadedStats) and reused below for the deduct —
        // must not be independently re-resolved at each site, or the two could disagree about
        // which pool to hit (system-architect finding, per_player_stat_pools.md).
        let cost_resolution = slot.cost.as_ref().map(|cost| {
            resolve_cost_source(&cost.stat, player_stats, loaded_stats.as_deref())
        });
        if let (Some(cost), Some((current, _))) = (&slot.cost, &cost_resolution) {
            if *current < cost.amount {
                events.push((key_str, GameEvent::Trigger(
                    format!("action_bar.insufficient_resource:{}", key_str),
                )));
                continue;
            }
        }

        // ── {target} check ────────────────────────────────────────────────────
        let needs_target = slot.do_actions.iter().any(action_needs_target);
        if needs_target && player_target.0.is_none() {
            events.push((key_str, GameEvent::Trigger(
                format!("action_bar.no_target:{}", key_str),
            )));
            continue;
        }

        // ── Fire ─────────────────────────────────────────────────────────────
        let target_id = player_target.0.as_deref().unwrap_or("");

        // Emit the intent event. The interpreter checks for a matching rule this frame;
        // if one matches, flush_pending_intent_system suppresses the slot's built-in do_actions.
        events.push((key_str, GameEvent::Trigger(
            format!("intent.slot.{}:{}", key_str, spawn_id.0),
        )));

        // Store pending actions (target-rewritten) + cooldown. Flushed to ActionQueue by
        // flush_pending_intent_system unless a rule handled the intent.
        // Cooldown and `action_bar.activated` are also deferred to flush so that a
        // suppressed intent never starts the cooldown or fires the result event.
        let mut actions: Vec<Action> = slot.do_actions.iter()
            .map(|a| rewrite_target(a.clone(), target_id))
            .collect();
        if let (Some(cost), Some((_, use_player_pool))) = (&slot.cost, &cost_resolution) {
            let key = if *use_player_pool {
                format!("{}.{}", spawn_id.0, cost.stat)
            } else {
                cost.stat.clone()
            };
            actions.push(Action::ModifyStat { key, delta: -cost.amount });
        }
        pending.0.insert(key_str.to_string(), (actions, slot.cooldown_secs));

        // action_bar.pressed fires immediately (before interpreter) — notification that the key
        // was pressed and passed all gate checks. Use for telemetry or UI feedback that should
        // fire regardless of whether a rule later cancels the intent.
        events.push((key_str, GameEvent::Trigger(
            format!("action_bar.pressed:{}", key_str),
        )));
    }

    // Emit in slot-key order, not query order: `slots.iter()` follows entity/archetype order, which
    // is not stable across machines or runs, and the interpreters read these events in write order.
    // Buffered (a `Vec` only allocates once an event is pushed, so an idle frame costs nothing) and
    // stable-sorted so one slot's own events keep their relative order. Two bars sharing a slot
    // key (already a validate error) tie and keep query order.
    events.sort_by(|a, b| a.0.cmp(b.0));
    for (_, event) in events {
        game_events.write(event);
    }
}

/// Whether the player entity carrying `idx` (its `PlayerIndex`, if any) owns a slot whose bar
/// declared `owner_player`. `None`/`Some(0)` both mean "the primary player" — matches
/// `is_primary_player`'s existing "`PlayerIndex(0)` or no `PlayerIndex` at all" definition.
fn owns_slot(owner_player: Option<u32>, idx: Option<&PlayerIndex>) -> bool {
    match owner_player {
        None | Some(0) => is_primary_player(idx),
        Some(n) => idx.is_some_and(|i| i.0 == n),
    }
}

/// Resolves a `SlotCost.stat` against the acting player's own `StatMap` first, falling back to
/// the global `LoadedStats` resource exactly as before per-player stat pools existed. Returns the
/// current value plus whether the player's own pool was the source — deliberately does **not**
/// build the dot-routed deduct key here (that requires a `format!` allocation): this is called
/// every frame from `action_bar_visual_system` for every cost-gated slot, so building a string
/// that call site immediately discards would allocate on a per-frame hot path for no reason
/// (wasm-perf-reviewer finding). The one call site that actually needs the key
/// (`action_bar_input_system`'s deduct push) builds it itself, already gated behind
/// `just_pressed` — see that call site.
///
/// Called once per firing slot and the result reused for both the cost gate and the deduct
/// action's key (see the two call sites in `action_bar_input_system` and
/// `action_bar_visual_system`) — computing this independently at each site risks the two
/// disagreeing about which pool a slot's cost resolves against. See
/// `planning/features/per_player_stat_pools.md`.
fn resolve_cost_source(
    stat: &str,
    player_stats: Option<&StatMap>,
    loaded_stats: Option<&LoadedStats>,
) -> (f32, bool) {
    if let Some(live) = player_stats.and_then(|sm| sm.0.get(stat)) {
        return (live.current, true);
    }
    let current = loaded_stats
        .and_then(|ls| ls.0.get(stat))
        .map(|s| s.current)
        .unwrap_or(0.0);
    (current, false)
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
    // `take` (not `drain`, which `BTreeMap` lacks): iterate in sorted slot-key order, leave empty.
    for (slot_key, (actions, cooldown)) in std::mem::take(&mut pending.0) {
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
    players: Query<(Option<&PlayerIndex>, Option<&StatMap>), With<CharacterController>>,
    mut overlays: Query<(&CooldownOverlay, &mut BackgroundColor)>,
) {
    for (slot, children) in &slots {
        let cd_frac = cooldowns.0.get(&slot.slot_key)
            .map(|(remaining, total)| (remaining / total).clamp(0.0, 1.0))
            .unwrap_or(0.0);

        // Same per-player-first, global-fallback resolution as action_bar_input_system's cost
        // check, so the dim overlay never disagrees with whether the slot will actually fire.
        // Only the current value is needed here, never the dot-routed deduct key — resolve_cost_
        // source is written so this never allocates a string on this per-frame path.
        let cost_ok = slot.cost.as_ref().map(|c| {
            let player_stats = players.iter()
                .find(|(idx, _)| owns_slot(slot.owner_player, *idx))
                .and_then(|(_, ps)| ps);
            resolve_cost_source(&c.stat, player_stats, loaded_stats.as_deref()).0 >= c.amount
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
        Action::SetDespawnTimer { entity, .. } => entity.contains("{target}"),
        Action::EmitEvent(s) => s.contains("{target}"),
        Action::PlayAnimationOn { target, .. } => target.contains("{target}"),
        Action::Spawn { at_entity, .. } => at_entity.as_deref().is_some_and(|e| e.contains("{target}")),
        _ => false,
    }
}

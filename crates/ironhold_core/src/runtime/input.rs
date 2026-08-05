use bevy::prelude::*;
use bevy::input::gamepad::{Gamepad, GamepadAxis};
use std::collections::{HashMap, HashSet};
use crate::runtime::messages::*;
use crate::runtime::scene_manager::{
    LoadedKeyBindings, LoadedGamepadBindings, PendingJoinGamepad, PendingEntitySpawns,
};
use crate::capabilities::player::{BoundGamepad, CharacterController, PlayerIndex};
use crate::schema::AppState;
use crate::schema::player::InputMap;

/// How long a stuck gamepad-binding situation (a bound player's pad has disappeared, or a pending
/// player's seed keeps resolving to a pad another player already holds) must persist before
/// `gamepad_bind_system` logs its one-shot diagnostic `warn!`. Not a design-critical value —
/// see `planning/features/gamepad_player_binding_hardening.md`'s "Open questions".
const GAMEPAD_DIAGNOSTIC_WARN_SECS: f32 = 3.0;

/// A candidate gamepad must have been continuously present (matching `Query<(Entity, &Gamepad)>`
/// without interruption) for at least this long before a pending player's seed is allowed to bind
/// to it. Exists because of a real, hardware-confirmed failure mode found during this feature's
/// own playtest: a single physical Xbox controller can report as **two** separate browser gamepad
/// entries for a brief moment (a `bevy_gilrs`/browser-level artifact — the second, spurious entry
/// disconnects on its own shortly after) — without this debounce, a pending player's seed could
/// permanently lock onto that spurious entry in the brief window before it disappears, since
/// binding is otherwise immediate and (by design) never re-derived once committed. Real hardware
/// showed this isn't a rare unlucky case: the spurious entry reliably wins the lower sorted
/// position (it's discovered first) and reliably outlives at least one `gamepad_bind_system` tick,
/// so without a debounce this reproduced on every single connection attempt with the affected
/// controller, not just occasionally. Not a design-critical exact value; picked to comfortably
/// exceed the observed spurious-entry lifetime without being perceptible as input lag on an
/// ordinary single, stable connection.
const GAMEPAD_STABLE_CONNECT_SECS: f32 = 0.5;

/// Translates global key presses into UI messages using the project's `global_key_bindings`.
/// Only fires in `InGame` state. Runs in `Update` (not `FixedUpdate`) so it
/// responds every rendered frame regardless of physics tick rate.
pub fn global_input_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    state: Res<State<AppState>>,
    key_bindings: Res<LoadedKeyBindings>,
    mut ui_events: MessageWriter<UiEvent>,
) {
    if *state.get() != AppState::InGame { return; }

    for (key_name, trigger) in &key_bindings.0 {
        if let Some(key_code) = InputMap::parse_key(key_name) {
            if keyboard_input.just_pressed(key_code) {
                ui_events.write(UiEvent::ButtonPressed(trigger.clone()));
            }
        } else {
            // Unknown key name — silently skip. A warning is logged at project load time
            // in check_project_loaded so the designer gets early feedback without spamming.
        }
    }
}

/// Detects a `join`-style press (or any other `global_unclaimed_gamepad_bindings`/
/// `scene_unclaimed_gamepad_bindings` trigger) on an **unclaimed** gamepad and, for the specific
/// purpose of `Action::JoinPlayer` binding the right physical pad to a freshly-joined player,
/// records which pad triggered it in `PendingJoinGamepad`. Only fires in `InGame` state. Must run
/// `.before(message_interpreter_system)` so `Action::JoinPlayer`'s executor sees this frame's
/// value, not last frame's.
///
/// No separate "live signal" prefilter is needed: a phantom/dead duplicate gamepad entry (the
/// documented Xbox 360 dual-registration quirk — see `docs/20_data_formats.md`) reports zero for
/// every button forever, so it can never produce the `just_pressed` edge this system requires —
/// requiring that edge on the specifically-bound button already excludes it.
///
/// `PendingJoinGamepad` is unconditionally reset to `None` at the top of every run, before any
/// new match is considered, so it never carries a stale pad identity across frames (e.g. from a
/// non-join gamepad trigger, like a pause button, that no `Action::JoinPlayer` consumes).
///
/// **At most one (pad, button) match is serviced per frame, full stop — capped at emission, not
/// just at capture.** `PendingJoinGamepad` can only hold one `Entity`, and nothing downstream can
/// tell which of several same-frame `UiEvent::ButtonPressed` messages a captured pad belongs to
/// (`message_interpreter_system` has no concept of "this message paired with that resource
/// value") — so emitting more than one qualifying event in a frame this system also captures a
/// pad for would silently mispair, or worse, both events could resolve to `Action::JoinPlayer`
/// and spawn two players from one pad's worth of same-frame pairing capacity (debug-detective /
/// system-architect finding: an earlier version capped only the `PendingJoinGamepad` write, not
/// the `ui_events.write` call, so a second unclaimed pad's simultaneous press still produced its
/// own `Action::JoinPlayer` with no pad bound — a permanently half-controlled player, since v1 has
/// no hot-leave to undo it). The loop stops (`break` out to the pad loop) the instant the first
/// match is found this frame — deterministic: lowest `Entity::index()`-sorted pad first, and
/// among that pad's own bindings, `HashMap` iteration order (irrelevant in practice: only matters
/// if one pad has two different bound buttons pressed the same frame). A second pad's press, or a
/// second *different* trigger on any pad, this same frame is simply not serviced — not queued, not
/// delayed, just dropped for this frame; the player presses again next frame.
pub fn unclaimed_gamepad_trigger_system(
    state: Res<State<AppState>>,
    gamepad_bindings: Res<LoadedGamepadBindings>,
    gamepad_query: Query<(Entity, &Gamepad)>,
    players: Query<(&BoundGamepad, &CharacterController)>,
    pending_spawns: Res<PendingEntitySpawns>,
    mut pending_join_gamepad: ResMut<PendingJoinGamepad>,
    mut ui_events: MessageWriter<UiEvent>,
) {
    pending_join_gamepad.0 = None;

    if *state.get() != AppState::InGame { return; }
    if gamepad_bindings.0.is_empty() { return; }

    let mut sorted_gamepads: Vec<(Entity, &Gamepad)> = gamepad_query.iter().collect();
    sorted_gamepads.sort_by_key(|(e, _)| e.index());

    // A pad is "claimed" if it drives a live player (via that player's resolved `BoundGamepad`),
    // or is already mid-flight through the deferred spawn queue via an undrained `is_hot_join`
    // entry's own captured `bound_gamepad` (mirrors the `queued_hot_joins` same-frame double-join
    // guard `Action::JoinPlayer`'s executor already has). `Entity`-based, not the old positional
    // `HashSet<usize>` derived from live `gamepad_index` values — that set went stale the instant
    // any pad connected/disconnected mid-session (e.g. a hot-leave shifting every remaining pad's
    // sorted position), risking a spurious extra join on an already-claimed pad. See
    // `planning/features/gamepad_player_binding_hardening.md`.
    let mut claimed: HashSet<Entity> = players.iter()
        .filter_map(|(bound, _)| bound.0)
        .chain(
            pending_spawns.0.iter()
                .filter(|q| q.is_hot_join)
                .filter_map(|q| q.player_config.as_ref().and_then(|pc| pc.bound_gamepad))
        )
        .collect();

    // Also reserve the pad a still-pending authored player's seed is *about* to resolve to.
    // `gamepad_bind_system` (`FixedUpdate`) is what actually writes `BoundGamepad`, but the fixed
    // timestep accumulator can tick zero times in a given frame — so on the very frame a pad first
    // becomes visible (its first press, which on the web is also this system's join-trigger
    // frame), a pending player whose seed resolves to that pad would otherwise look unclaimed for
    // one frame here and lose it to a spurious join before `gamepad_bind_system` ever gets to bind
    // them (debug-detective finding, post-implementation review). Positional on purpose — this is
    // exactly the one-frame gap `gamepad_bind_system` hasn't closed yet, not a live re-derivation
    // of an already-bound player's position.
    let sorted_entities: Vec<Entity> = sorted_gamepads.iter().map(|(e, _)| *e).collect();
    for (bound, controller) in &players {
        if bound.0.is_some() { continue; }
        if let Some(seed) = controller.inputs.gamepad_index {
            if let Some(&e) = sorted_entities.get(seed) {
                claimed.insert(e);
            }
        }
    }

    for (entity, gamepad) in sorted_gamepads.iter() {
        if claimed.contains(entity) { continue; }

        let matched_trigger = gamepad_bindings.0.iter().find_map(|(button_name, trigger)| {
            let button = InputMap::parse_gamepad_button(button_name)?;
            gamepad.just_pressed(button).then(|| trigger.clone())
        });

        if let Some(trigger) = matched_trigger {
            ui_events.write(UiEvent::ButtonPressed(trigger));
            pending_join_gamepad.0 = Some(*entity);
            break;
        }
    }
}

/// Resolves each pending player's `BoundGamepad` once, using `InputMap.gamepad_index` purely as a
/// one-time seed against the current frame's sorted-by-`Entity::index()` gamepad slice. Once a
/// player binds, their `BoundGamepad` is never touched again (barring a future hot-leave/rejoin) —
/// see `BoundGamepad`'s doc comment in `capabilities/player.rs`. Ordered `.before(
/// input_translator_system)` so a player who binds this tick already has gamepad input applied
/// the same tick.
///
/// Visits every player in one pass — not a per-player branch folded into another system — because
/// of a hard invariant: it must never bind a player to a gamepad `Entity` any other player's
/// `BoundGamepad` already holds. Without this, a cross-*time* race is possible: pad B connects
/// first and binds to P1 (seed 0); P2 (seed 1) is out of range, stays pending; pad A connects
/// later with a *lower* `Entity::index()` than B; the sorted slice becomes `[A, B]`; P2's seed 1
/// now resolves to B — already bound to P1. `claimed` starts from every already-bound player, plus
/// every undrained hot-join spawn's own captured `bound_gamepad` (mirrors
/// `unclaimed_gamepad_trigger_system`'s equivalent chain — a hot-joined player can sit in
/// `PendingEntitySpawns` for one or more frames since `drain_spawn_queue_system` is rate-limited,
/// and without this a pending scene player could bind to the *same* pad in that window, producing
/// two live players on one controller; system-architect/debug-detective finding, post-
/// implementation review) — and grows as this same pass binds new ones, so two players sharing a
/// duplicated `gamepad_index` in one scene can also never both claim the same pad within a single
/// frame (see `planning/features/gamepad_player_binding_hardening.md`'s cross-time-race fix and
/// duplicate-detection sections). Pending candidates are visited in ascending `PlayerIndex` order
/// (not raw query/archetype order) so which player wins a duplicated seed is deterministic and
/// reproducible, not an accident of component layout.
///
/// A displaced pending player is never auto-rebound to a different, already-free pad this session
/// (see the feature plan's "Explicitly out of scope") — it just stays pending, diagnosed by the
/// one-shot `warn!` below once the stuck state persists past `GAMEPAD_DIAGNOSTIC_WARN_SECS`. A
/// seed that simply doesn't resolve to any connected pad (the ordinary "no gamepad plugged in yet"
/// case) is never warned about at all — that's expected, silent, keyboard-only play. Same
/// treatment for a candidate pad that exists but hasn't been continuously present for
/// `GAMEPAD_STABLE_CONNECT_SECS` yet (see that constant's doc comment) — ordinary, silent, no
/// diagnostic; expected to resolve within a fraction of a second in the common case.
pub fn gamepad_bind_system(
    mut query: Query<(Entity, Option<&PlayerIndex>, &CharacterController, &mut BoundGamepad)>,
    gamepad_query: Query<(Entity, &Gamepad)>,
    pending_spawns: Res<PendingEntitySpawns>,
    time: Res<Time>,
    mut stuck_secs: Local<HashMap<Entity, f32>>,
    mut warned: Local<HashSet<Entity>>,
    mut stable_secs: Local<HashMap<Entity, f32>>,
) {
    let mut sorted_gamepads: Vec<Entity> = gamepad_query.iter().map(|(e, _)| e).collect();
    sorted_gamepads.sort_by_key(|e| e.index());
    let connected: HashSet<Entity> = sorted_gamepads.iter().copied().collect();

    // How long each currently-connected gamepad has been continuously present, without
    // interruption — reset to zero (via the `retain` below dropping its entry) the instant it
    // disappears from `gamepad_query`, so a pad that disconnects and reconnects starts its
    // stability clock over rather than picking up where it left off.
    for &e in &sorted_gamepads {
        *stable_secs.entry(e).or_insert(0.0) += time.delta_secs();
    }
    stable_secs.retain(|e, _| connected.contains(e));

    let mut claimed: HashSet<Entity> = query.iter().filter_map(|(_, _, _, bound)| bound.0)
        .chain(
            pending_spawns.0.iter()
                .filter(|q| q.is_hot_join)
                .filter_map(|q| q.player_config.as_ref().and_then(|pc| pc.bound_gamepad))
        )
        .collect();

    // Stale `Local` entries for despawned players (scene unload, a future hot-leave) would
    // otherwise persist for the rest of the session — harmless (Bevy's entity generation prevents
    // a recycled index from false-matching), just an unbounded leak. Pruned once per call, cheap:
    // both `retain` calls are no-ops in the common case where nothing is currently stuck.
    if !stuck_secs.is_empty() {
        stuck_secs.retain(|&e, _| query.contains(e));
    }
    if !warned.is_empty() {
        warned.retain(|&e| query.contains(e));
    }

    let mut pending_order: Vec<(u32, Entity)> = query.iter()
        .filter(|(_, _, _, bound)| bound.0.is_none())
        .map(|(e, idx, _, _)| (idx.map(|i| i.0).unwrap_or(0), e))
        .collect();
    pending_order.sort_by_key(|(idx, _)| *idx);

    for (player_entity, player_index, _controller, bound) in &query {
        let Some(gp_entity) = bound.0 else { continue };
        if connected.contains(&gp_entity) {
            stuck_secs.remove(&player_entity);
            warned.remove(&player_entity);
        } else {
            let elapsed = stuck_secs.entry(player_entity).or_insert(0.0);
            *elapsed += time.delta_secs();
            if *elapsed >= GAMEPAD_DIAGNOSTIC_WARN_SECS && warned.insert(player_entity) {
                warn!(
                    "Player P{}: gamepad disconnected — reconnect it to the same port/slot to \
                     resume gamepad input (keyboard bindings, if any, are unaffected).",
                    player_index.map(|i| i.0).unwrap_or(0) + 1
                );
            }
        }
    }

    for (_, player_entity) in pending_order {
        let Ok((_, player_index, controller, mut bound)) = query.get_mut(player_entity) else { continue };
        let Some(seed) = controller.inputs.gamepad_index else { continue };
        let Some(&gp_entity) = sorted_gamepads.get(seed) else { continue };

        if claimed.contains(&gp_entity) {
            let elapsed = stuck_secs.entry(player_entity).or_insert(0.0);
            *elapsed += time.delta_secs();
            if *elapsed >= GAMEPAD_DIAGNOSTIC_WARN_SECS && warned.insert(player_entity) {
                warn!(
                    "Player P{}: gamepad_index {} resolves to a controller already bound to \
                     another player — staying on keyboard. Give this player a different \
                     gamepad_index.",
                    player_index.map(|i| i.0).unwrap_or(0) + 1, seed
                );
            }
            continue;
        }

        if stable_secs.get(&gp_entity).copied().unwrap_or(0.0) < GAMEPAD_STABLE_CONNECT_SECS {
            continue;
        }

        bound.0 = Some(gp_entity);
        claimed.insert(gp_entity);
        stuck_secs.remove(&player_entity);
        warned.remove(&player_entity);
    }
}

pub fn input_translator_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    query: Query<(Entity, &CharacterController, Option<&BoundGamepad>)>,
    gamepad_query: Query<&Gamepad>,
    mut input_events: MessageWriter<InputActionMessage>,
    #[cfg(feature = "inspector")]
    inspector_enabled: Option<Res<crate::inspector::InspectorEnabled>>,
) {
    #[cfg(feature = "inspector")]
    if let Some(enabled) = inspector_enabled {
        if enabled.0 {
            return;
        }
    }

    for (entity, controller, bound) in &query {
        // A player's `BoundGamepad` is resolved once by `gamepad_bind_system` and never
        // re-derived from a live sorted position here — immune to any other pad's
        // connect/disconnect churn. `None` (still pending, no gamepad_index authored, or the
        // component simply isn't present on this entity — e.g. a minimal test-constructed player)
        // simply means no gamepad input this tick; keyboard bindings are unaffected either way.
        let gamepad = bound.and_then(|b| b.0).and_then(|e| gamepad_query.get(e).ok());
        let deadzone = controller.inputs.gamepad_deadzone;

        let strafe_mode = controller.inputs.strafe_mouse_button
            .as_deref()
            .and_then(InputMap::parse_mouse_button)
            .map(|btn| mouse_input.pressed(btn))
            .unwrap_or(false);
        let mut move_vec = Vec2::ZERO;
        let mut turn = 0.0;

        if let Some(key) = controller.inputs.key("forward") {
            if keyboard_input.pressed(key) { move_vec.y += 1.0; }
        }
        if let Some(key) = controller.inputs.key("backward") {
            if keyboard_input.pressed(key) { move_vec.y -= 1.0; }
        }
        // A/D strafe only when left mouse is held; otherwise A/D rotates instead.
        if strafe_mode {
            if let Some(key) = controller.inputs.key("right") {
                if keyboard_input.pressed(key) { move_vec.x += 1.0; }
            }
            if let Some(key) = controller.inputs.key("left") {
                if keyboard_input.pressed(key) { move_vec.x -= 1.0; }
            }
        } else {
            if let Some(key) = controller.inputs.key("left") {
                if keyboard_input.pressed(key) { turn += 1.0; }
            }
            if let Some(key) = controller.inputs.key("right") {
                if keyboard_input.pressed(key) { turn -= 1.0; }
            }
        }

        // Gamepad, when bound: left stick strafes/moves, right stick turns — independent of
        // the keyboard's strafe_mode (that toggle only exists to disambiguate A/D on a
        // keyboard; a gamepad already has separate sticks for move and turn).
        if let Some(gp) = gamepad {
            let lx = gp.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
            let ly = gp.get(GamepadAxis::LeftStickY).unwrap_or(0.0);
            if lx.abs() > deadzone { move_vec.x += lx; }
            if ly.abs() > deadzone { move_vec.y += ly; }

            let rx = gp.get(GamepadAxis::RightStickX).unwrap_or(0.0);
            if rx.abs() > deadzone { turn -= rx; }
        }

        // `clamp_length_max` (not `normalize`) so a fully-analog gamepad tilt keeps its
        // magnitude — only a diagonal keyboard press (length > 1) gets scaled down.
        if move_vec != Vec2::ZERO {
            input_events.write(InputActionMessage {
                entity,
                action: InputAction::Move(move_vec.clamp_length_max(1.0)),
            });
        }

        if turn != 0.0 {
            input_events.write(InputActionMessage {
                entity,
                action: InputAction::Turn(turn),
            });
        }

        let gamepad_jump = controller.inputs.gamepad_button("jump");
        let jump_pressed = controller.inputs.key("jump")
            .map(|k| keyboard_input.just_pressed(k))
            .unwrap_or(false)
            || gamepad.zip(gamepad_jump).map(|(gp, btn)| gp.just_pressed(btn)).unwrap_or(false);
        if jump_pressed {
            input_events.write(InputActionMessage {
                entity,
                action: InputAction::Jump(true),
            });
        }

        let gamepad_run = controller.inputs.gamepad_button("run");
        let run_pressed = controller.inputs.key("run")
            .map(|k| keyboard_input.just_pressed(k))
            .unwrap_or(false)
            || gamepad.zip(gamepad_run).map(|(gp, btn)| gp.just_pressed(btn)).unwrap_or(false);
        if run_pressed {
            input_events.write(InputActionMessage {
                entity,
                action: InputAction::Run(true),
            });
        }
    }
}

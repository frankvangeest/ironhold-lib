use bevy::prelude::*;
use bevy::input::gamepad::{Gamepad, GamepadAxis};
use crate::runtime::messages::*;
use crate::runtime::scene_manager::{
    LoadedKeyBindings, LoadedGamepadBindings, PendingJoinGamepad, PendingEntitySpawns,
};
use crate::capabilities::player::CharacterController;
use crate::schema::AppState;
use crate::schema::player::InputMap;

/// Resolves a player's `gamepad_index` against a slice of connected gamepads already sorted by
/// `Entity::index()` (built once per system per frame — see callers). `None` if the player has
/// no `gamepad_index` bound, or the index is out of range (fewer gamepads connected than
/// expected).
pub(crate) fn resolve_gamepad<'a>(
    sorted: &'a [(Entity, &'a Gamepad)],
    index: Option<usize>,
) -> Option<&'a Gamepad> {
    index.and_then(|i| sorted.get(i)).map(|(_, gp)| *gp)
}

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
    controllers: Query<&CharacterController>,
    pending_spawns: Res<PendingEntitySpawns>,
    mut pending_join_gamepad: ResMut<PendingJoinGamepad>,
    mut ui_events: MessageWriter<UiEvent>,
) {
    pending_join_gamepad.0 = None;

    if *state.get() != AppState::InGame { return; }
    if gamepad_bindings.0.is_empty() { return; }

    let mut sorted_gamepads: Vec<(Entity, &Gamepad)> = gamepad_query.iter().collect();
    sorted_gamepads.sort_by_key(|(e, _)| e.index());

    // A pad is "claimed" if it drives a live player, or is already mid-flight through the
    // deferred spawn queue via an undrained `is_hot_join` entry (mirrors the `queued_hot_joins`
    // same-frame double-join guard `Action::JoinPlayer`'s executor already has).
    //
    // Known accepted hazard (system-architect finding, not fixed here): this set holds
    // *positional* sorted indices, recomputed fresh each frame — if a lower-sorted-index pad
    // disconnects mid-session, every higher pad's position shifts down by one, and a still-live
    // player's own claimed index can transiently collide with a now-different physical pad, or a
    // claimed index can point past the end of the (now shorter) list and stop excluding anything.
    // Either way a stale `gamepad_index` can make an actually-claimed pad look unclaimed for one
    // frame, risking a spurious extra join on a pad someone is already using. Same root cause as
    // the pre-existing RON-authored `gamepad_index` fragility (`resolve_gamepad`'s doc comment),
    // just newly reachable via a live disconnect instead of only via authoring. Logged in
    // `planning/backlog.md` rather than solved here — the real fix is an `Entity`-resolved binding,
    // not a positional index.
    let claimed: std::collections::HashSet<usize> = controllers.iter()
        .filter_map(|c| c.inputs.gamepad_index)
        .chain(
            pending_spawns.0.iter()
                .filter(|q| q.is_hot_join)
                .filter_map(|q| q.player_config.as_ref().and_then(|pc| pc.inputs.gamepad_index))
        )
        .collect();

    for (sorted_index, (entity, gamepad)) in sorted_gamepads.iter().enumerate() {
        if claimed.contains(&sorted_index) { continue; }

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

pub fn input_translator_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    query: Query<(Entity, &CharacterController)>,
    gamepad_query: Query<(Entity, &Gamepad)>,
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

    // Bevy has no built-in numeric gamepad index — each connected pad is its own entity.
    // Sort by entity index so `InputMap.gamepad_index: 0` consistently means "whichever
    // gamepad connected first this session" (good enough for local co-op; no rebinding UI).
    let mut sorted_gamepads: Vec<(Entity, &Gamepad)> = gamepad_query.iter().collect();
    sorted_gamepads.sort_by_key(|(e, _)| e.index());

    for (entity, controller) in &query {
        let gamepad = resolve_gamepad(&sorted_gamepads, controller.inputs.gamepad_index);
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

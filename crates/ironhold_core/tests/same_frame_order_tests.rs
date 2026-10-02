//! D1 (`planning/backlog.md` Beta 0.5): same-frame ordering of gameplay maps.
//!
//! Rust's std `HashMap`/`HashSet` use a seeded hasher (random on native; address-derived and so
//! allocation-history-dependent on wasm32), so their iteration order differs per map instance, per
//! run and per platform. These tests pin the four places where such an
//! order used to decide a simulation outcome (see
//! `planning/investigations/hashmap_iteration_order_audit.md`):
//!
//! - `PendingIntentActions` drain order -> `ActionQueue` push order (+ `intent.slot.*` event order)
//! - `LoadedStats` iteration -> stat threshold events -> first-matching FSM transition
//! - `LoadedKeyBindings` iteration -> `UiEvent` order
//! - `LoadedGamepadBindings` iteration -> which bound button wins on one pad
//!
//! Each test builds a fresh `App` several times (every `App` gets freshly seeded hashers), so a
//! regression to a hash-ordered container fails with high probability instead of occasionally.

use bevy::input::gamepad::GamepadButton;
use bevy::prelude::*;
use std::collections::BTreeMap;

use ironhold_core::capabilities::action_bar::ActionSlotUi;
use ironhold_core::capabilities::player::{CharacterController, PlayerTarget};
use ironhold_core::runtime::scene_manager::{LoadedGamepadBindings, LoadedKeyBindings};
use ironhold_core::runtime::{ActionQueue, GameEvent, SpawnId, UiEvent};
use ironhold_core::schema::{
    Action, AppState, LiveStat, LoadedStats, StatDef, StatThreshold, ThresholdCondition,
};

mod support;
use support::{connect_test_gamepad, press_gamepad_button, setup_test_app};

/// How many fresh `App`s (= fresh hasher seeds) each order test builds.
const ROUNDS: usize = 8;

// Copied from entity_logic_tests.rs (a shared test helper is a logged backlog item).
fn intent_test_player_controller() -> CharacterController {
    use ironhold_core::schema::player::InputMap;
    CharacterController {
        walk_speed: 5.0, run_speed: 8.0, rot_speed: 2.0,
        inputs: InputMap {
            forward: "KeyW".to_string(), backward: "KeyS".to_string(),
            left: "KeyA".to_string(), right: "KeyD".to_string(),
            strafe_left: "KeyQ".to_string(), strafe_right: "KeyE".to_string(),
            jump: "Space".to_string(), run: "ShiftLeft".to_string(),
            interact: "KeyF".to_string(), strafe_mouse_button: None,
            target_next: "Tab".to_string(), target_range: 30.0,
            gamepad_index: None, look_left: None, look_right: None, look_up: None, look_down: None,
            gamepad_jump: "South".to_string(), gamepad_run: "East".to_string(),
            gamepad_interact: "West".to_string(), gamepad_target_next: "North".to_string(),
            gamepad_deadzone: 0.15,
        },
        is_running: false, jump_velocity: 5.94, double_jump_enabled: false,
        double_jump_velocity: 5.94, jumps_used: 0, max_jumps: 1,
        collider_radius: 0.4, ground_cast_length: 0.3, max_walkable_slope_deg: 45.0, coyote_time_secs: 0.1, coyote_ticks_remaining: 0, idle_drag: 0.8, jump_air_grace: 0, jump_liftoff_y: None,
    }
}


fn game_event_names(app: &App) -> Vec<String> {
    app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .map(|e| {
            let GameEvent::Trigger(n) = e;
            n.clone()
        })
        .collect()
}

fn ui_trigger_names(app: &App) -> Vec<String> {
    app.world()
        .resource::<Messages<UiEvent>>()
        .iter_current_update_messages()
        .map(|e| {
            let UiEvent::ButtonPressed(t) = e;
            t.clone()
        })
        .collect()
}

// ── PendingIntentActions / action_bar_input_system ──────────────────────────────────────────

fn slot(key: &str, keycode: KeyCode, delta: f32) -> ActionSlotUi {
    ActionSlotUi {
        slot_key: key.to_string(),
        resolved_key: Some(keycode),
        resolved_gamepad_button: None,
        do_actions: vec![Action::ModifyStat { key: "health".to_string(), delta }],
        cooldown_secs: None,
        cost: None,
        owner_player: None,
    }
}

/// Two slots fire in the same frame: a heal (`+50`) on slot "1" and a sacrifice (`-60`) on slot
/// "2", against a stat that clamps after every step (80/100). Pushed in slot-key order the result
/// is 80 -> 100 (clamped) -> 40; pushed in the reverse order it is 80 -> 20 -> 70. The old
/// `HashMap` drain gave either, depending on the hasher seed. Slot spawn order must not matter.
#[test]
fn simultaneous_slots_push_actions_and_emit_events_in_slot_key_order() {
    for spawn_sacrifice_first in [false, true] {
        for _ in 0..ROUNDS {
            let mut app = setup_test_app();
            app.update();

            let mut health = LiveStat::new(StatDef {
                base: 100.0,
                min: 0.0,
                max: 100.0,
                soft_max: None,
                regen_rate: 0.0,
                regen_delay: 0.0,
                thresholds: vec![],
            });
            health.current = 80.0;
            let mut stats = LoadedStats::default();
            stats.0.insert("health".to_string(), health);
            app.world_mut().insert_resource(stats);

            let heal = slot("1", KeyCode::Digit1, 50.0);
            let sacrifice = slot("2", KeyCode::Digit2, -60.0);
            if spawn_sacrifice_first {
                app.world_mut().spawn(sacrifice);
                app.world_mut().spawn(heal);
            } else {
                app.world_mut().spawn(heal);
                app.world_mut().spawn(sacrifice);
            }
            app.world_mut().spawn((
                SpawnId("player_01".to_string()),
                intent_test_player_controller(),
                PlayerTarget::default(),
            ));

            {
                let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                keys.press(KeyCode::Digit1);
                keys.press(KeyCode::Digit2);
            }
            app.update();

            let health_now = app.world().resource::<LoadedStats>().0["health"].current;
            assert_eq!(
                health_now, 40.0,
                "slot actions must apply in slot-key order (heal then sacrifice = 40); got {health_now} \
                 (spawn_sacrifice_first = {spawn_sacrifice_first})"
            );

            let names = game_event_names(&app);
            let pos = |needle: &str| names.iter().position(|n| n == needle);
            let (p1, p2) = (pos("intent.slot.1:player_01"), pos("intent.slot.2:player_01"));
            assert!(
                p1.is_some() && p2.is_some() && p1 < p2,
                "intent events must be emitted in slot-key order regardless of spawn/query order; \
                 got {names:?}"
            );
            let (a1, a2) = (pos("action_bar.activated:1"), pos("action_bar.activated:2"));
            assert!(
                a1.is_some() && a2.is_some() && a1 < a2,
                "activated events must follow slot-key order; got {names:?}"
            );
        }
    }
}

// ── LoadedStats / stat_threshold_system ─────────────────────────────────────────────────────

/// Three stats cross their thresholds in the same frame. The threshold events reach the FSM in
/// write order and the FSM takes the first matching transition, so the order must follow the
/// stats' declaration order (the `LoadedStats` `IndexMap`), never a hash order.
#[test]
fn simultaneous_stat_thresholds_fire_in_declaration_order() {
    let orders: [[&str; 3]; 4] = [
        ["zeta", "alpha", "mid"],
        ["alpha", "mid", "zeta"],
        ["mid", "zeta", "alpha"],
        ["alpha", "zeta", "mid"],
    ];
    for order in orders {
        for _ in 0..ROUNDS {
            let mut app = setup_test_app();
            app.update();

            let mut stats = LoadedStats::default();
            for name in order {
                stats.0.insert(
                    name.to_string(),
                    LiveStat::new(StatDef {
                        base: 10.0,
                        min: 0.0,
                        max: 10.0,
                        soft_max: None,
                        regen_rate: 0.0,
                        regen_delay: 0.0,
                        thresholds: vec![StatThreshold {
                            when: ThresholdCondition::BelowOrEqual(0.0),
                            emit: format!("t.{name}"),
                        }],
                    }),
                );
            }
            app.world_mut().insert_resource(stats);
            for name in order {
                app.world_mut()
                    .resource_mut::<ActionQueue>()
                    .push(Action::ModifyStat { key: name.to_string(), delta: -10.0 });
            }
            app.update();

            let fired: Vec<String> =
                game_event_names(&app).into_iter().filter(|n| n.starts_with("t.")).collect();
            let expected: Vec<String> = order.iter().map(|n| format!("t.{n}")).collect();
            assert_eq!(
                fired, expected,
                "stat threshold events must fire in LoadedStats declaration order {order:?}"
            );
        }
    }
}

// ── LoadedKeyBindings / global_input_system ─────────────────────────────────────────────────

/// Two or more bound keys pressed in the same frame produce their `UiEvent`s in key-name order,
/// not hash order (e.g. a pause key and an inventory key pressed together).
#[test]
fn simultaneous_key_bindings_fire_in_key_name_order() {
    for _ in 0..ROUNDS {
        let mut app = setup_test_app();
        app.update();
        app.world_mut().insert_resource(State::new(AppState::InGame));
        app.world_mut().insert_resource(LoadedKeyBindings(BTreeMap::from([
            ("KeyZ".to_string(), "trig_z".to_string()),
            ("KeyA".to_string(), "trig_a".to_string()),
            ("KeyM".to_string(), "trig_m".to_string()),
        ])));
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::KeyZ);
            keys.press(KeyCode::KeyA);
            keys.press(KeyCode::KeyM);
        }
        app.update();

        let fired: Vec<String> =
            ui_trigger_names(&app).into_iter().filter(|t| t.starts_with("trig_")).collect();
        assert_eq!(
            fired,
            vec!["trig_a", "trig_m", "trig_z"],
            "bound keys pressed in one frame must emit UiEvents in key-name order"
        );
    }
}

// ── LoadedGamepadBindings / unclaimed_gamepad_trigger_system ────────────────────────────────

/// One unclaimed pad presses two different bound buttons in the same frame: exactly one trigger
/// is serviced, and it is the first by button name ("East" < "South"), every time.
#[test]
fn unclaimed_pad_with_two_bound_buttons_pressed_services_the_first_by_button_name() {
    for _ in 0..ROUNDS {
        let mut app = setup_test_app();
        app.update();
        app.world_mut().insert_resource(State::new(AppState::InGame));
        app.world_mut().insert_resource(LoadedGamepadBindings(BTreeMap::from([
            ("South".to_string(), "pad_south".to_string()),
            ("East".to_string(), "pad_east".to_string()),
        ])));

        let gamepad = connect_test_gamepad(&mut app);
        app.update();
        press_gamepad_button(&mut app, gamepad, GamepadButton::South);
        press_gamepad_button(&mut app, gamepad, GamepadButton::East);
        app.update();

        let fired: Vec<String> =
            ui_trigger_names(&app).into_iter().filter(|t| t.starts_with("pad_")).collect();
        assert_eq!(
            fired,
            vec!["pad_east"],
            "one pad, two bound buttons pressed together: only the first by button name is serviced"
        );
    }
}

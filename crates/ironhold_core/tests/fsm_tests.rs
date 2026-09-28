use bevy::prelude::*;
use ironhold_core::runtime::{UiEvent, ActionQueue, SceneEvent, LoadedStateMachine, LogicState};
use ironhold_core::schema::{AppState, Action, StateMachineAsset, FsmState, FsmTransition, FsmEventBinding};

mod support;
use support::setup_test_app;

/// Sentinel written to `DebugState.last_action` before a negative-assertion phase of a test, so
/// "nothing fired" can be verified as "the sentinel is still there" rather than accidentally
/// passing because a *previous* phase's real action is still sitting in `last_action` unchanged
/// (a stale value in a single mutable slot reads the same either way, so the two cases are only
/// distinguishable if the slot is reset to something neither phase would ever produce).
const RESET_MARKER: &str = "__test_reset_marker__";

/// Helper: build a minimal StateMachineAsset with two states ("a" and "b") and one transition.
fn make_test_fsm() -> StateMachineAsset {
    StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![
            FsmState {
                name: "a".to_string(),
                entry_actions: vec![Action::Log("entered_a".to_string())],
                exit_actions:  vec![Action::Log("exited_a".to_string())],
                on: vec![
                    FsmEventBinding {
                        event: "ui.button_pressed:in_state_a".to_string(),
                        do_actions: vec![Action::Log("in_state_a_fired".to_string())],
                    },
                ],
            },
            FsmState {
                name: "b".to_string(),
                entry_actions: vec![Action::Log("entered_b".to_string())],
                exit_actions:  vec![Action::Log("exited_b".to_string())],
                on: vec![],
            },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("a".to_string()),
                on: "ui.button_pressed:go_b".to_string(),
                to: "b".to_string(),
            },
        ],
        global_on: vec![
            FsmEventBinding {
                event: "ui.button_pressed:global_action".to_string(),
                do_actions: vec![Action::Log("global_fired".to_string())],
            },
        ],
    }
}

#[test]
fn test_action_to_state_transition() {
    let mut app = setup_test_app();

    // 1. Run once to handle Startup
    app.update();

    // 2. Transition to InGame
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::InGame);
    app.update(); // Set transition
    app.update(); // Apply transition

    {
        let state = app.world().resource::<State<AppState>>();
        assert_eq!(*state.get(), AppState::InGame);
    }

    // 3. Manually push an action
    app.world_mut().resource_mut::<ActionQueue>().0.push_back(Action::LoadScene("scenes/tests/another_scene.ron".to_string()));

    // 4. Run executor
    app.update(); // Executor sets NextState
    app.update(); // Apply transition

    // 5. Verify state transitioned to LoadingScene
    let state = app.world().resource::<State<AppState>>();
    assert_eq!(*state.get(), AppState::LoadingScene);
}

#[test]
fn test_global_on_fires_action() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    // Fire a global event
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("global_action".to_string()));
    app.update();

    // action_executor_system runs chained right after the interpreter in the same Update pass,
    // so the queue is already drained by the time we get here -- check DebugState.last_action
    // instead (same pattern as every other action-firing assertion in this file).
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"global_fired\")");
}

#[test]
fn test_fsm_transition_fires_exit_enter_and_advances_state() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    // Fire event that triggers transition a -> b
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_b".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "Transition should advance LogicState to the target state");

    // FIFO exit->entry: last executed action should be the entry action of state b.
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"entered_b\")");
}

#[test]
fn test_state_gated_on_binding_only_fires_in_matching_state() {
    let mut app = setup_test_app();
    app.update();

    // In-state binding in "a" should not fire when in "b"
    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    // Fire event for state "a" binding while in "a" - should fire
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("in_state_a".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"in_state_a_fired\")");

    // Change state to "b" - should NOT fire. Reset the sentinel first: `last_action` is a single
    // mutable slot, so if nothing fires it just stays at whatever the PREVIOUS phase left there
    // (the assertion above's own value) -- indistinguishable from "correctly suppressed" without
    // resetting to a value neither phase would ever produce.
    app.world_mut().resource_mut::<LogicState>().0 = "b".to_string();
    app.world_mut().resource_mut::<ironhold_core::DebugState>().last_action = RESET_MARKER.to_string();

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("in_state_a".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, RESET_MARKER,
        "In-state on binding must be suppressed in wrong state");
}

#[test]
fn test_fsm_transition_does_not_fire_from_wrong_state() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    // Start in "b" -- transition is from "a" only.
    app.world_mut().insert_resource(LogicState("b".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_b".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "Transition with from:Some(\"a\") must not fire from state \"b\"");
}

#[test]
fn test_fsm_in_state_on_binding_suppressed_in_wrong_state() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    // Start in "b" -- the "in_state_a" binding belongs to "a".
    app.world_mut().insert_resource(LogicState("b".to_string()));
    app.world_mut().resource_mut::<ironhold_core::DebugState>().last_action = RESET_MARKER.to_string();

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("in_state_a".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "State must not change");
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, RESET_MARKER,
        "In-state on binding must be suppressed in wrong state");
}

#[test]
fn test_transition_advances_logic_state() {
    let mut app = setup_test_app();
    app.update();

    let fsm = make_test_fsm();
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    // Fire transition event
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_b".to_string()));
    app.update();

    // LogicState should be updated
    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "Transition should advance LogicState");
}

#[test]
fn test_global_on_fires_before_state_transition() {
    let mut app = setup_test_app();
    app.update();

    // "b" has an entry action, so the LAST action executed proves ordering: if global_on's action
    // is still the last thing executed, the transition's entry action never ran (or ran first and
    // got overwritten) -- either way this test would fail, so asserting "entered_b" specifically
    // proves global_on fired *before* the transition's entry action within the same frame, not
    // merely that "something" fired.
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![
            FsmState { name: "a".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState { name: "b".to_string(), entry_actions: vec![Action::Log("entered_b".to_string())], exit_actions: vec![], on: vec![] },
        ],
        transitions: vec![
            FsmTransition { from: Some("a".to_string()), on: "ui.button_pressed:go_b".to_string(), to: "b".to_string() },
        ],
        global_on: vec![
            FsmEventBinding { event: "ui.button_pressed:go_b".to_string(), do_actions: vec![Action::Log("global_fired_first".to_string())] },
        ],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_b".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"entered_b\")",
        "global_on's action must execute before the transition's entry action within the same frame");
    assert_eq!(app.world().resource::<LogicState>().0, "b", "transition should still advance state");
}

#[test]
fn test_fsm_exit_before_entry_fifo_order() {
    // Verifies: exit actions run before entry actions, and declaration order is preserved
    // within each group. State "a" has two exit actions; state "b" has two entry actions.
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![
            FsmState {
                name: "a".to_string(),
                entry_actions: vec![],
                exit_actions: vec![
                    Action::Log("exit_a_1".to_string()),
                    Action::Log("exit_a_2".to_string()),
                ],
                on: vec![],
            },
            FsmState {
                name: "b".to_string(),
                entry_actions: vec![
                    Action::Log("entry_b_1".to_string()),
                    Action::Log("entry_b_2".to_string()),
                ],
                exit_actions: vec![],
                on: vec![],
            },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("a".to_string()),
                on: "ui.button_pressed:go".to_string(),
                to: "b".to_string(),
            },
        ],
        global_on: vec![],
    };

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go".to_string()));
    app.update();

    // FIFO execution order: exit_a_1, exit_a_2, entry_b_1, entry_b_2.
    // last_action reflects the final action executed.
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"entry_b_2\")",
        "FIFO exit->entry: last executed action should be the second entry action of state b");
}

#[test]
fn test_fsm_exit_action_fires_on_transition() {
    // "b" has no entry actions, so last_action after the transition reflects the exit action.
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![
            FsmState {
                name: "a".to_string(),
                entry_actions: vec![],
                exit_actions: vec![Action::Log("exited_a".to_string())],
                on: vec![],
            },
            FsmState {
                name: "b".to_string(),
                entry_actions: vec![],
                exit_actions: vec![],
                on: vec![],
            },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("a".to_string()),
                on: "ui.button_pressed:go".to_string(),
                to: "b".to_string(),
            },
        ],
        global_on: vec![],
    };

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b");
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"exited_a\")",
        "Exit action should be the last executed action when target state has no entry actions");
}

// ── SceneEvent tests ──────────────────────────────────────────────────────────
//
// All scene-event payloads below use a realistic scene-path shape
// ("projects/test/scenes/{stem}.scene.ron"), not a bare stem string -- a `scene_path_stem`
// implementation that just echoed its input verbatim (the bug fixed in 7ef4df9) would still pass
// tests that fed it an already-bare stem, which is exactly why that regression shipped with zero
// test coverage catching it.

#[test]
fn test_scene_ready_event_fires_actions() {
    let mut app = setup_test_app();
    app.update();

    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "".to_string(),
        states: vec![],
        transitions: vec![],
        global_on: vec![
            FsmEventBinding { event: "scene.ready:test_scene".to_string(), do_actions: vec![Action::Log("scene_ready_fired".to_string())] },
        ],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("".to_string()));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Ready("projects/test/scenes/test_scene.scene.ron".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"scene_ready_fired\")");
}

#[test]
fn test_scene_loaded_event_fires_actions() {
    let mut app = setup_test_app();
    app.update();

    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "".to_string(),
        states: vec![],
        transitions: vec![],
        global_on: vec![
            FsmEventBinding { event: "scene.loaded:test_scene".to_string(), do_actions: vec![Action::Log("scene_loaded_fired".to_string())] },
        ],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("".to_string()));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Loaded("projects/test/scenes/test_scene.scene.ron".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"scene_loaded_fired\")");
}

#[test]
fn test_scene_requested_event_fires_actions() {
    let mut app = setup_test_app();
    app.update();

    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "".to_string(),
        states: vec![],
        transitions: vec![],
        global_on: vec![
            FsmEventBinding { event: "scene.requested:test_scene".to_string(), do_actions: vec![Action::Log("scene_requested_fired".to_string())] },
        ],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("".to_string()));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Requested("projects/test/scenes/test_scene.scene.ron".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"scene_requested_fired\")");
}

#[test]
fn test_scene_unloading_event_fires_actions() {
    let mut app = setup_test_app();
    app.update();

    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "".to_string(),
        states: vec![],
        transitions: vec![],
        global_on: vec![
            FsmEventBinding { event: "scene.unloading:test_scene".to_string(), do_actions: vec![Action::Log("scene_unloading_fired".to_string())] },
        ],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("".to_string()));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Unloading("projects/test/scenes/test_scene.scene.ron".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"scene_unloading_fired\")");
}

#[test]
fn test_fsm_scene_event_triggers_transition() {
    let mut fsm = make_test_fsm();
    // Any-state transition triggered by a scene ready event.
    fsm.transitions.push(FsmTransition {
        from: None,
        on: "scene.ready:main".to_string(),
        to: "b".to_string(),
    });

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Ready("projects/test/scenes/main.scene.ron".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "SceneEvent::Ready should trigger an FSM transition");
}

#[test]
fn test_fsm_scene_event_triggers_in_state_on_binding() {
    let mut fsm = make_test_fsm();
    // Add a scene.ready binding to state "a".
    fsm.states[0].on.push(FsmEventBinding {
        event: "scene.ready:start_menu".to_string(),
        do_actions: vec![Action::Log("scene_ready_in_state_a".to_string())],
    });

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Ready("projects/test/scenes/start_menu.scene.ron".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"scene_ready_in_state_a\")",
        "SceneEvent should trigger an in-state on binding when in the matching state");
}

#[test]
fn test_fsm_scene_event_loaded_triggers_transition() {
    let mut fsm = make_test_fsm();
    fsm.transitions.push(FsmTransition {
        from: None,
        on: "scene.loaded:main".to_string(),
        to: "b".to_string(),
    });

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Loaded("projects/test/scenes/main.scene.ron".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "SceneEvent::Loaded should trigger an FSM transition");
}

#[test]
fn test_fsm_no_loaded_state_machine_is_noop() {
    let mut app = setup_test_app();
    app.update();

    // Explicit None -- no FSM loaded.
    app.world_mut().insert_resource(LoadedStateMachine(None));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("any_event".to_string()));
    app.update(); // must not panic

    let queue = app.world().resource::<ActionQueue>();
    assert!(queue.0.is_empty(), "No FSM loaded -- action queue must remain empty");
    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "", "LogicState must remain unchanged when no FSM is loaded");
}

// ── ActionQueue FIFO ordering ─────────────────────────────────────────────────

#[test]
fn test_action_queue_is_fifo() {
    // Actions pushed first must execute first.
    let mut app = setup_test_app();
    app.update();

    let mut queue = app.world_mut().resource_mut::<ActionQueue>();
    queue.0.push_back(Action::Log("first".to_string()));
    queue.0.push_back(Action::Log("second".to_string()));
    queue.0.push_back(Action::Log("third".to_string()));

    app.update(); // executor runs

    let queue = app.world().resource::<ActionQueue>();
    assert!(queue.0.is_empty(), "Queue should be empty after executor runs");
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"third\")",
        "FIFO: last pushed action should be last executed (last_action reflects final execution)");
}

#[test]
fn test_multiple_transitions_same_frame_only_first_fires() {
    let mut app = setup_test_app();
    app.update();

    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![
            FsmState { name: "a".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState { name: "b".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState { name: "c".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
        ],
        transitions: vec![
            FsmTransition { from: Some("a".to_string()), on: "ui.button_pressed:go_b".to_string(), to: "b".to_string() },
            FsmTransition { from: Some("a".to_string()), on: "ui.button_pressed:go_b".to_string(), to: "c".to_string() },
        ],
        global_on: vec![],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_b".to_string()));
    app.update();

    // Only first matching transition should fire
    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "Only first matching transition should fire");
}

#[test]
fn test_transition_with_no_from_matches_any_state() {
    let mut app = setup_test_app();
    app.update();

    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![
            FsmState { name: "a".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState { name: "b".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
        ],
        transitions: vec![
            FsmTransition { from: None, on: "ui.button_pressed:go_b".to_string(), to: "b".to_string() },
        ],
        global_on: vec![],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_b".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "Transition with no from should match any state");
}

#[test]
fn test_global_on_fires_regardless_of_state() {
    let mut app = setup_test_app();
    app.update();

    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![
            FsmState { name: "a".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState { name: "b".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
        ],
        transitions: vec![],
        global_on: vec![
            FsmEventBinding { event: "ui.button_pressed:global".to_string(), do_actions: vec![Action::Log("global_works".to_string())] },
        ],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    // Should fire in any state
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("global".to_string()));
    app.update();
    assert_eq!(app.world().resource::<LogicState>().0, "a");
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"global_works\")");

    // Change state, should still fire. Reset the sentinel first so this second phase's assertion
    // proves a fresh fire happened, not just that the first phase's value is still sitting there.
    app.world_mut().resource_mut::<LogicState>().0 = "b".to_string();
    app.world_mut().resource_mut::<ironhold_core::DebugState>().last_action = RESET_MARKER.to_string();

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("global".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"global_works\")",
        "global_on must fire again from state \"b\", not merely retain state \"a\"'s stale result");
}

#[test]
fn test_fsm_state_advance_visible_in_same_frame() {
    // Two events arrive in the same frame.
    // Event 1 "go_b" fires the a->b transition and advances logic_state to "b" immediately.
    // Event 2 "go_c" fires the b->c transition because the interpreter already sees state "b".
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![
            FsmState { name: "a".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState { name: "b".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState { name: "c".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("a".to_string()),
                on: "ui.button_pressed:go_b".to_string(),
                to: "b".to_string(),
            },
            FsmTransition {
                from: Some("b".to_string()),
                on: "ui.button_pressed:go_c".to_string(),
                to: "c".to_string(),
            },
        ],
        global_on: vec![],
    };

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    // Both events in the same frame -- first advances state so second can fire.
    app.world_mut()
        .resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_b".to_string()));
    app.world_mut()
        .resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_c".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "c",
        "State advance from first transition must be visible to second event in the same frame");
}

#[test]
fn test_minimal_fsm_parses() {
    let ron_str = r#"
        (
            schema_version: 1,
            global_on: [
                ( event: "scene.ready:main", do_actions: [ Log("Scene ready") ] ),
            ],
        )
    "#;
    let fsm: StateMachineAsset = ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(ron_str)
        .expect("Minimal global_on FSM must parse");
    assert_eq!(fsm.schema_version, 1);
    assert_eq!(fsm.initial_state, "");
    assert!(fsm.states.is_empty());
    assert!(fsm.transitions.is_empty());
    assert_eq!(fsm.global_on.len(), 1);
    assert!(fsm.validate().is_ok(), "Minimal global_on FSM must validate");
}

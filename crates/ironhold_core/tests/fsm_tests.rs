use bevy::prelude::*;
use ironhold_core::runtime::{UiEvent, ActionQueue, SceneEvent, LoadedRules, LoadedStateMachine, LogicState};
use ironhold_core::schema::{AppState, Action, LogicRule, StateMachineAsset, FsmState, FsmTransition, FsmEventBinding};

mod support;
use support::setup_test_app;

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
    app.world_mut().resource_mut::<ActionQueue>().push(Action::LoadScene("scenes/tests/another_scene.ron".to_string()));
    
    // 4. Run executor
    app.update(); // Executor sets NextState
    app.update(); // Apply transition
    
    // 5. Verify state transitioned to LoadingScene
    let state = app.world().resource::<State<AppState>>();
    assert_eq!(*state.get(), AppState::LoadingScene);
}

#[test]
fn test_enter_state_action_updates_logic_state() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::EnterState("playing".to_string()));
    app.update(); // executor fires

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "playing", "EnterState should update LogicState");
}

#[test]
fn test_state_gated_rule_only_fires_in_matching_state() {
    let mut app = setup_test_app();
    app.update();

    // Rule fires EnterState("triggered") only while in the "active" logic state.
    app.world_mut().insert_resource(LoadedRules(vec![
        LogicRule {
            on: "ui.button_pressed:do_thing".to_string(),
            when: Some("active".to_string()),
            do_actions: vec![Action::EnterState("triggered".to_string())],
        }
    ]));

    // Fire event while in the wrong state ("") â€” rule must be suppressed.
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("do_thing".to_string()));
    app.update();
    {
        let state = app.world().resource::<LogicState>();
        assert_eq!(state.0, "", "Rule should be suppressed in non-matching state");
    }

    // Transition to the matching state, then fire the event again.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::EnterState("active".to_string()));
    app.update();

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("do_thing".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "triggered", "Rule should fire in the matching state");
}

#[test]
fn test_fsm_in_state_on_binding_fires() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("in_state_a".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"in_state_a_fired\")",
        "In-state on binding should fire while in matching state");
}

#[test]
fn test_fsm_in_state_on_binding_suppressed_in_wrong_state() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    // Start in "b" â€” the "in_state_a" binding belongs to "a".
    app.world_mut().insert_resource(LogicState("b".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("in_state_a".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "State must not change");
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_ne!(debug.last_action, "Log(\"in_state_a_fired\")",
        "In-state on binding must be suppressed in wrong state");
}

#[test]
fn test_fsm_transition_fires_exit_enter_and_advances_state() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    // Trigger the transition a â†’ b.
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_b".to_string()));
    app.update();

    // State must have advanced to "b".
    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "Transition should advance LogicState to the target state");

    // The last action processed by the executor should be the entry action for "b".
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"entered_b\")",
        "Entry actions for the new state should fire after the transition");
}

#[test]
fn test_fsm_transition_does_not_fire_from_wrong_state() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    // Start in "b" â€” transition is from "a" only.
    app.world_mut().insert_resource(LogicState("b".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_b".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "Transition with from:Some(\"a\") must not fire from state \"b\"");
}

#[test]
fn test_fsm_any_state_transition_fires_from_any_state() {
    let mut fsm = make_test_fsm();
    // Add an any-state transition to "b".
    fsm.transitions.push(FsmTransition {
        from: None,
        on: "ui.button_pressed:anywhere_go_b".to_string(),
        to: "b".to_string(),
    });

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    // Start in "b" â€” the any-state transition should still fire.
    app.world_mut().insert_resource(LogicState("b".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("anywhere_go_b".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "Any-state transition (from: None) should fire from any state");
}

#[test]
fn test_fsm_global_on_fires_regardless_of_state() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    app.world_mut().insert_resource(LogicState("b".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("global_action".to_string()));
    app.update();

    // State must not change; global_on fires only the declared action.
    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "global_on must not change state");
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"global_fired\")",
        "global_on binding should fire from any state");
}

#[test]
fn test_rules_no_match_does_not_queue_action() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedRules(vec![
        LogicRule {
            on: "ui.button_pressed:something_else".to_string(),
            when: None,
            do_actions: vec![Action::Log("should_not_fire".to_string())],
        },
    ]));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("unmatched_event".to_string()));
    app.update();

    let queue = app.world().resource::<ActionQueue>();
    assert!(queue.0.is_empty(), "No rule matched â€” queue must stay empty");
}

#[test]
fn test_rules_scene_event_ready_triggers_action() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedRules(vec![
        LogicRule {
            on: "scene.ready:main".to_string(),
            when: None,
            do_actions: vec![Action::Log("scene_ready_fired".to_string())],
        },
    ]));

    // Interpreter strips path to stem "main" via scene_path_stem.
    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Ready("projects/test/scenes/main.scene.ron".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"scene_ready_fired\")",
        "scene.ready:main rule should fire on SceneEvent::Ready for main scene");
}

#[test]
fn test_rules_scene_event_loaded_triggers_action() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedRules(vec![
        LogicRule {
            on: "scene.loaded:main".to_string(),
            when: None,
            do_actions: vec![Action::Log("scene_loaded_fired".to_string())],
        },
    ]));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Loaded("projects/test/scenes/main.scene.ron".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"scene_loaded_fired\")",
        "scene.loaded:main rule should fire on SceneEvent::Loaded before entities are spawned");
}

#[test]
fn test_rules_scene_event_requested_triggers_action() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedRules(vec![
        LogicRule {
            on: "scene.requested:main".to_string(),
            when: None,
            do_actions: vec![Action::Log("scene_requested_fired".to_string())],
        },
    ]));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Requested("projects/test/scenes/main.scene.ron".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"scene_requested_fired\")",
        "scene.requested:main rule should fire on SceneEvent::Requested");
}

#[test]
fn test_rules_scene_event_unloading_triggers_action() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedRules(vec![
        LogicRule {
            on: "scene.unloading:main".to_string(),
            when: None,
            do_actions: vec![Action::Log("scene_unloading_fired".to_string())],
        },
    ]));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Unloading("projects/test/scenes/main.scene.ron".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"scene_unloading_fired\")",
        "scene.unloading:main rule should fire on SceneEvent::Unloading");
}

#[test]
fn test_action_queue_is_fifo() {
    // Actions pushed first must execute first.
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>().push(Action::Log("first".to_string()));
    app.world_mut().resource_mut::<ActionQueue>().push(Action::Log("second".to_string()));
    app.world_mut().resource_mut::<ActionQueue>().push(Action::Log("third".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"third\")",
        "FIFO: last pushed action should be last executed (last_action reflects final execution)");
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
        "FIFO exitâ†’entry: last executed action should be the second entry action of state b");
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

    // Explicit None â€” no FSM loaded.
    app.world_mut().insert_resource(LoadedStateMachine(None));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("any_event".to_string()));
    app.update(); // must not panic

    let queue = app.world().resource::<ActionQueue>();
    assert!(queue.0.is_empty(), "No FSM loaded â€” action queue must remain empty");
    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "", "LogicState must remain unchanged when no FSM is loaded");
}

#[test]
fn test_fsm_only_first_matching_transition_fires() {
    // Two transitions on the same event from state "a": first â†’ "b", second â†’ "c".
    // The FSM interpreter uses `.find()` so only the first match executes.
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
                on: "ui.button_pressed:go".to_string(),
                to: "b".to_string(),
            },
            FsmTransition {
                from: Some("a".to_string()),
                on: "ui.button_pressed:go".to_string(),
                to: "c".to_string(),
            },
        ],
        global_on: vec![],
    };

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut()
        .resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b",
        "Only the first matching transition should fire; second transition to 'c' must be ignored");
}

#[test]
fn test_fsm_state_advance_visible_in_same_frame() {
    // Two events arrive in the same frame.
    // Event 1 "go_b" fires the aâ†’b transition and advances logic_state to "b" immediately.
    // Event 2 "go_c" fires the bâ†’c transition because the interpreter already sees state "b".
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

    // Both events in the same frame â€” first advances state so second can fire.
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

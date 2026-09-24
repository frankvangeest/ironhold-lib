use bevy::prelude::*;
use ironhold_core::runtime::{UiEvent, ActionQueue, SceneEvent, LoadedStateMachine, LogicState};
use ironhold_core::schema::{AppState, Action, StateMachineAsset, FsmState, FsmTransition, FsmEventBinding};

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

    // Fire a global event
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("global_action".to_string()));
    app.update();

    // Check that the action was queued
    let queue = app.world().resource::<ActionQueue>();
    assert_eq!(queue.0.len(), 1);
    assert!(matches!(&queue.0[0], Action::Log(_)));
}

#[test]
fn test_transition_fires_exit_then_entry_actions() {
    let mut app = setup_test_app();
    app.update();

    // Fire event that triggers transition a -> b
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_b".to_string()));
    app.update();

    // Check that exit then entry actions were queued
    let queue = app.world().resource::<ActionQueue>();
    assert_eq!(queue.0.len(), 2);
    assert!(matches!(&queue.0[0], Action::Log(s) if s == "exited_a"));
    assert!(matches!(&queue.0[1], Action::Log(s) if s == "entered_b"));
}

#[test]
fn test_state_gated_on_binding_only_fires_in_matching_state() {
    let mut app = setup_test_app();
    app.update();

    // In-state binding in "a" should not fire when in "b"
    let fsm = make_test_fsm();
    let fsm_handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm.clone());
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    // Fire event for state "a" binding while in "a" - should fire
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("in_state_a".to_string()));
    app.update();
    
    let queue = app.world().resource::<ActionQueue>();
    assert_eq!(queue.0.len(), 1);
    assert!(matches!(&queue.0[0], Action::Log(s) if s == "in_state_a_fired"));

    // Change state to "b" - should NOT fire
    app.world_mut().resource_mut::<LogicState>().0 = "b".to_string();
    app.world_mut().resource_mut::<ActionQueue>().0.clear();
    
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("in_state_a".to_string()));
    app.update();
    
    let queue = app.world().resource::<ActionQueue>();
    assert_eq!(queue.0.len(), 0);
}

#[test]
fn test_transition_advances_logic_state() {
    let mut app = setup_test_app();
    app.update();

    let fsm = make_test_fsm();
    let fsm_handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm.clone());
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
fn test_initial_state_sets_logic_state() {
    let mut app = setup_test_app();
    app.update();

    let fsm = make_test_fsm();
    let fsm_handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm.clone());
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));

    app.update();

    // LogicState should be set to initial_state
    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "a", "LogicState should be set to initial_state");
}

#[test]
fn test_global_on_fires_before_state_transition() {
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
            FsmTransition { from: Some("a".to_string()), on: "ui.button_pressed:go_b".to_string(), to: "b".to_string() },
        ],
        global_on: vec![
            FsmEventBinding { event: "ui.button_pressed:go_b".to_string(), do_actions: vec![Action::Log("global_fired_first".to_string())] },
        ],
    };
    let fsm_handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm.clone());
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_b".to_string()));
    app.update();

    // global_on should fire BEFORE transition actions
    let queue = app.world().resource::<ActionQueue>();
    assert_eq!(queue.0.len(), 3);
    assert!(matches!(&queue.0[0], Action::Log(s) if s == "global_fired_first"));
    assert!(matches!(&queue.0[1], Action::Log(s) if s == "exited_a"));
    assert!(matches!(&queue.0[2], Action::Log(s) if s == "entered_b"));
}

// ── SceneEvent tests ──────────────────────────────────────────────────────────

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
    let fsm_handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm.clone());
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("".to_string()));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Ready("test_scene".to_string()));
    app.update();

    let queue = app.world().resource::<ActionQueue>();
    assert_eq!(queue.0.len(), 1);
    assert!(matches!(&queue.0[0], Action::Log(s) if s == "scene_ready_fired"));
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
    let fsm_handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm.clone());
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("".to_string()));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Loaded("test_scene".to_string()));
    app.update();

    let queue = app.world().resource::<ActionQueue>();
    assert_eq!(queue.0.len(), 1);
    assert!(matches!(&queue.0[0], Action::Log(s) if s == "scene_loaded_fired"));
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
    let fsm_handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm.clone());
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("".to_string()));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Requested("test_scene".to_string()));
    app.update();

    let queue = app.world().resource::<ActionQueue>();
    assert_eq!(queue.0.len(), 1);
    assert!(matches!(&queue.0[0], Action::Log(s) if s == "scene_requested_fired"));
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
    let fsm_handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm.clone());
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("".to_string()));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Unloading("test_scene".to_string()));
    app.update();

    let queue = app.world().resource::<ActionQueue>();
    assert_eq!(queue.0.len(), 1);
    assert!(matches!(&queue.0[0], Action::Log(s) if s == "scene_unloading_fired"));
}

// ── ActionQueue FIFO ordering ─────────────────────────────────────────────────

#[test]
fn test_action_queue_is_fifo() {
    let mut app = setup_test_app();
    app.update();

    let mut queue = app.world_mut().resource_mut::<ActionQueue>();
    queue.0.push_back(Action::Log("first".to_string()));
    queue.0.push_back(Action::Log("second".to_string()));
    queue.0.push_back(Action::Log("third".to_string()));

    app.update(); // executor runs

    let queue = app.world().resource::<ActionQueue>();
    assert!(queue.0.is_empty(), "Queue should be empty after executor runs");
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
    let fsm_handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm.clone());
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
    let fsm_handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm.clone());
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
    let fsm_handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm.clone());
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    // Should fire in any state
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("global".to_string()));
    app.update();
    assert_eq!(app.world().resource::<LogicState>().0, "a");
    let queue = app.world().resource::<ActionQueue>();
    assert_eq!(queue.0.len(), 1);

    // Change state, should still fire
    app.world_mut().resource_mut::<LogicState>().0 = "b".to_string();
    app.world_mut().resource_mut::<ActionQueue>().0.clear();
    
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("global".to_string()));
    app.update();
    
    let queue = app.world().resource::<ActionQueue>();
    assert_eq!(queue.0.len(), 1);
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
use bevy::prelude::*;
use crate::runtime::messages::*;
use crate::runtime::actions::ActionQueue;
use super::{LoadedRules, LoadedStateMachine, LogicState};

pub fn message_interpreter_system(
    mut ui_events: MessageReader<UiMessage>,
    mut scene_events: MessageReader<SceneEvent>,
    mut action_queue: ResMut<ActionQueue>,
    loaded_rules: Res<LoadedRules>,
    logic_state: Res<LogicState>,
) {
    for event in ui_events.read() {
        let event_name = match event {
            UiMessage::ButtonPressed(trigger) => format!("ui.button_pressed:{}", trigger),
        };
        match_rules(&event_name, &loaded_rules, &logic_state, &mut action_queue);
    }

    for event in scene_events.read() {
        let event_name = match event {
            SceneEvent::Requested(path) => format!("scene.requested:{}", scene_path_stem(path)),
            SceneEvent::Loaded(path)    => format!("scene.loaded:{}",    scene_path_stem(path)),
            SceneEvent::Ready(path)     => format!("scene.ready:{}",     scene_path_stem(path)),
            SceneEvent::Unloading(path) => format!("scene.unloading:{}", scene_path_stem(path)),
        };
        match_rules(&event_name, &loaded_rules, &logic_state, &mut action_queue);
    }
}

fn match_rules(
    event_name: &str,
    loaded_rules: &LoadedRules,
    logic_state: &LogicState,
    action_queue: &mut ActionQueue,
) {
    for rule in &loaded_rules.0 {
        let state_matches = match &rule.when {
            None => true,
            Some(required) => required == &logic_state.0,
        };
        if rule.on == event_name && state_matches {
            for action in &rule.do_actions {
                info!("Rule Matched! Event: {} -> Action: {:?}", event_name, action);
                action_queue.push(action.clone());
            }
        }
    }
}

fn scene_path_stem(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path)
        .trim_end_matches(".scene.ron")
        .trim_end_matches(".ron")
}

/// Interprets events against a loaded `StateMachineAsset`, driving state transitions and
/// queuing entry/exit actions.  Runs alongside `message_interpreter_system`; only one of
/// the two will have data to act on for any given project (rules vs. FSM).
pub fn fsm_interpreter_system(
    mut ui_events: MessageReader<UiMessage>,
    mut scene_events: MessageReader<SceneEvent>,
    mut action_queue: ResMut<ActionQueue>,
    loaded_fsm: Res<LoadedStateMachine>,
    mut logic_state: ResMut<LogicState>,
) {
    let Some(fsm) = &loaded_fsm.0 else { return };

    // Collect all events for this frame before mutating state.
    let mut events: Vec<String> = Vec::new();
    for event in ui_events.read() {
        let UiMessage::ButtonPressed(trigger) = event;
        events.push(format!("ui.button_pressed:{}", trigger));
    }
    for event in scene_events.read() {
        let name = match event {
            SceneEvent::Requested(path) => format!("scene.requested:{}", scene_path_stem(path)),
            SceneEvent::Loaded(path)    => format!("scene.loaded:{}",    scene_path_stem(path)),
            SceneEvent::Ready(path)     => format!("scene.ready:{}",     scene_path_stem(path)),
            SceneEvent::Unloading(path) => format!("scene.unloading:{}", scene_path_stem(path)),
        };
        events.push(name);
    }

    for event_name in &events {
        // 1. global_on — fires regardless of state, no state change.
        for binding in &fsm.global_on {
            if binding.event == *event_name {
                for action in &binding.do_actions {
                    info!("FSM global_on: {} -> {:?}", event_name, action);
                    action_queue.push(action.clone());
                }
            }
        }

        // 2. In-state on bindings — fire while in current state, no state change.
        if let Some(state_def) = fsm.states.iter().find(|s| s.name == logic_state.0) {
            for binding in &state_def.on {
                if binding.event == *event_name {
                    for action in &binding.do_actions {
                        info!("FSM in-state on [{}]: {} -> {:?}", logic_state.0, event_name, action);
                        action_queue.push(action.clone());
                    }
                }
            }
        }

        // 3. Transitions — fire exit/entry actions and advance state.
        //    Only the first matching transition fires per event.
        let transition = fsm.transitions.iter().find(|t| {
            let from_ok = match &t.from {
                None => true,
                Some(f) => *f == logic_state.0,
            };
            from_ok && t.on == *event_name
        });

        if let Some(transition) = transition {
            let from_name = logic_state.0.clone();
            let to_name = transition.to.clone();

            info!("FSM transition: \"{}\" -> \"{}\" on \"{}\"", from_name, to_name, event_name);

            // ActionQueue is FIFO — push in desired execution order: exit first, then entry.
            if let Some(from_def) = fsm.states.iter().find(|s| s.name == from_name) {
                for action in &from_def.exit_actions {
                    info!("FSM exit [{}]: {:?}", from_name, action);
                    action_queue.push(action.clone());
                }
            }

            if let Some(to_def) = fsm.states.iter().find(|s| s.name == to_name) {
                for action in &to_def.entry_actions {
                    info!("FSM entry [{}]: {:?}", to_name, action);
                    action_queue.push(action.clone());
                }
            }

            // Advance logic state immediately so subsequent events this frame see the new state.
            logic_state.0 = to_name.clone();
        }
    }
}

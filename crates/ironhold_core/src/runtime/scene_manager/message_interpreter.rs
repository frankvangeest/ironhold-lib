use bevy::prelude::*;
use crate::runtime::messages::*;
use crate::runtime::actions::ActionQueue;
use super::{LoadedRules, LogicState};

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
            SceneEvent::Ready(path) => {
                let stem = scene_path_stem(&path);
                format!("scene.ready:{}", stem)
            }
            SceneEvent::Unloading(path) => {
                let stem = scene_path_stem(&path);
                format!("scene.unloading:{}", stem)
            }
            _ => continue,
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

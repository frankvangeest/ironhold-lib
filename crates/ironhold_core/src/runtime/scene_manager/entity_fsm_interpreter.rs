use bevy::prelude::*;
use crate::runtime::messages::*;
use crate::runtime::actions::ActionQueue;
use crate::schema::Action;
use crate::capabilities::action_bar::{CurrentTarget, HandledIntentSlots};
use super::{LoadedStateMachine, BehaviorHandle, EntityFsmState, SpawnId};

use crate::runtime::scene_manager::action_substitution::{rewrite_self, rewrite_target, intent_slot_key, scene_path_stem};

/// Interprets events against entity behavior files (per-entity FSM).
/// Runs chained after `fsm_interpreter_system` and before `action_executor_system`.
pub fn entity_fsm_interpreter_system(
    mut ui_events: MessageReader<UiEvent>,
    mut game_events: MessageReader<GameEvent>,
    mut scene_events: MessageReader<SceneEvent>,
    mut action_queue: ResMut<ActionQueue>,
    mut entities: Query<(&BehaviorHandle, &mut EntityFsmState, &SpawnId)>,
    state_machines: Res<Assets<crate::schema::project::StateMachineAsset>>,
    current_target: Res<CurrentTarget>,
    mut handled_intents: ResMut<HandledIntentSlots>,
) {
    let target_id = current_target.0.as_deref().unwrap_or("");
    // Collect all events emitted this frame.
    let mut events: Vec<String> = Vec::new();
    for event in ui_events.read() {
        let UiEvent::ButtonPressed(trigger) = event;
        events.push(format!("ui.button_pressed:{}", trigger));
    }
    for event in game_events.read() {
        let GameEvent::Trigger(name) = event;
        events.push(name.clone());
    }
    for event in scene_events.read() {
        let name = match event {
            SceneEvent::Requested(p) => format!("scene.requested:{}", scene_path_stem(p).unwrap_or_default()),
            SceneEvent::Loaded(p)    => format!("scene.loaded:{}",    scene_path_stem(p).unwrap_or_default()),
            SceneEvent::Ready(p)     => format!("scene.ready:{}",     scene_path_stem(p).unwrap_or_default()),
            SceneEvent::Unloading(p) => format!("scene.unloading:{}", scene_path_stem(p).unwrap_or_default()),
        };
        events.push(name);
    }

    if events.is_empty() { return; }

    for (behavior, mut fsm_state, spawn_id) in &mut entities {
        let Some(fsm) = state_machines.get(&behavior.0) else { continue };
        let id = &spawn_id.0;

        for event_name in &events {
            let mut intent_matched = false;

            // global_on — fires from any state without changing state.
            for binding in &fsm.global_on {
                let pattern = binding.event.replace("{self}", id);
                if pattern == *event_name {
                    for action in &binding.do_actions {
                        info!("Entity FSM [{}] global_on: {} -> {:?}", id, event_name, action);
                        action_queue.push(rewrite_target(rewrite_self(action.clone(), id), target_id));
                    }
                    intent_matched = true;
                }
            }

            // In-state on bindings — fire while in current state, no state change.
            let current = fsm_state.current.clone();
            if let Some(state_def) = fsm.states.iter().find(|s| s.name == current) {
                for binding in &state_def.on {
                    let pattern = binding.event.replace("{self}", id);
                    if pattern == *event_name {
                        for action in &binding.do_actions {
                            info!("Entity FSM [{}] in-state on [{}]: {} -> {:?}",
                                id, current, event_name, action);
                            action_queue.push(rewrite_target(rewrite_self(action.clone(), id), target_id));
                        }
                        intent_matched = true;
                    }
                }
            }

            // Transitions — first match wins; pushes exit/entry actions and advances state.
            let transition = fsm.transitions.iter().find(|t| {
                let from_ok = t.from.as_ref().map_or(true, |f| *f == fsm_state.current);
                let pattern = t.on.replace("{self}", id);
                from_ok && pattern == *event_name
            });

            if let Some(transition) = transition {
                let from_name = fsm_state.current.clone();
                let to_name = transition.to.clone();

                info!(
                    "Entity FSM [{}]: \"{}\" -> \"{}\" on \"{}\"",
                    id, from_name, to_name, event_name
                );

                if let Some(from_def) = fsm.states.iter().find(|s| s.name == from_name) {
                    for action in &from_def.exit_actions {
                        info!("Entity FSM [{}] exit [{}]: {:?}", id, from_name, action);
                        action_queue.push(rewrite_target(rewrite_self(action.clone(), id), target_id));
                    }
                }
                if let Some(to_def) = fsm.states.iter().find(|s| s.name == to_name) {
                    for action in &to_def.entry_actions {
                        info!("Entity FSM [{}] entry [{}]: {:?}", id, to_name, action);
                        action_queue.push(rewrite_target(rewrite_self(action.clone(), id), target_id));
                    }
                }

                fsm_state.current = to_name;
                intent_matched = true;
            }

            if intent_matched {
                if let Some(slot_key) = intent_slot_key(event_name) {
                    handled_intents.0.insert(slot_key);
                }
            }
        }
    }
}
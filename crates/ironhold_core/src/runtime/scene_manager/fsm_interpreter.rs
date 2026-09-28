use bevy::prelude::*;
use crate::runtime::messages::*;
use crate::runtime::actions::ActionQueue;
use crate::schema::Action;
use crate::capabilities::action_bar::{CurrentTarget, HandledIntentSlots};
use crate::capabilities::player::PlayerTarget;
use crate::SpawnId;
use super::{LoadedStateMachine, LogicState};

use crate::runtime::scene_manager::action_substitution::{rewrite_target, scene_path_stem, intent_slot_key};

/// Interprets events against a loaded `StateMachineAsset`, driving state transitions and
/// queuing entry/exit actions.
/// 
/// Returns early when `LoadedStateMachine` is `None`, *before* draining its `MessageReader`s.
/// This means the project-load frame's `scene.requested:<initial>` event is never seen by the FSM
/// interpreter. No live content binds `scene.requested:` for the initial scene, so nothing
/// observable changes.
pub fn fsm_interpreter_system(
    mut ui_events: MessageReader<UiEvent>,
    mut game_events: MessageReader<GameEvent>,
    mut scene_events: MessageReader<SceneEvent>,
    mut action_queue: ResMut<ActionQueue>,
    loaded_fsm: Res<LoadedStateMachine>,
    mut logic_state: ResMut<LogicState>,
    current_target: Res<CurrentTarget>,
    mut handled_intents: ResMut<HandledIntentSlots>,
) {
    let Some(fsm) = &loaded_fsm.0 else { return };
    let target_id = current_target.0.as_deref().unwrap_or("");

    // Collect all events for this frame before mutating state.
    let mut events: Vec<String> = Vec::new();
    for event in ui_events.read() {
        let UiEvent::ButtonPressed(trigger) = event;
        events.push(format!("ui.button_pressed:{}", trigger));
    }
    for event in game_events.read() {
        let GameEvent::Trigger(name) = event;
        // Trigger names are used as-is; caller namespaces them (e.g. "entity.collected:coin_01").
        events.push(name.clone());
    }
    for event in scene_events.read() {
        let name = match event {
            SceneEvent::Requested(path) => format!("scene.requested:{}", scene_path_stem(path).unwrap_or_default()),
            SceneEvent::Loaded(path)    => format!("scene.loaded:{}",    scene_path_stem(path).unwrap_or_default()),
            SceneEvent::Ready(path)     => format!("scene.ready:{}",     scene_path_stem(path).unwrap_or_default()),
            SceneEvent::Unloading(path) => format!("scene.unloading:{}", scene_path_stem(path).unwrap_or_default()),
        };
        events.push(name);
    }

    let mut any_intent_matched = false;

    for event_name in &events {
        let mut intent_matched = false;

        // 1. global_on — fires regardless of state, no state change.
        for binding in &fsm.global_on {
            if binding.event == *event_name {
                for action in &binding.do_actions {
                    info!("FSM global_on: {} -> {:?}", event_name, action);
                    action_queue.push(rewrite_target(action.clone(), target_id));
                }
                intent_matched = true;
                any_intent_matched = true;
                // Mark intent slot as handled so action_bar doesn't also fire its built-in.
                if let Some(slot_key) = intent_slot_key(event_name) {
                    handled_intents.0.insert(slot_key);
                }
            }
        }

        // 2. In-state on bindings — fire while in current state, no state change.
        if let Some(state_def) = fsm.states.iter().find(|s| s.name == logic_state.0) {
            for binding in &state_def.on {
                if binding.event == *event_name {
                    for action in &binding.do_actions {
                        info!("FSM in-state on [{}]: {} -> {:?}", logic_state.0, event_name, action);
                        action_queue.push(rewrite_target(action.clone(), target_id));
                    }
                    intent_matched = true;
                    any_intent_matched = true;
                    // Mark intent slot as handled so action_bar doesn't also fire its built-in.
                    if let Some(slot_key) = intent_slot_key(event_name) {
                        handled_intents.0.insert(slot_key);
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
                    action_queue.push(rewrite_target(action.clone(), target_id));
                }
            }

            if let Some(to_def) = fsm.states.iter().find(|s| s.name == to_name) {
                for action in &to_def.entry_actions {
                    info!("FSM entry [{}]: {:?}", to_name, action);
                    action_queue.push(rewrite_target(action.clone(), target_id));
                }
            }

            // Advance logic state immediately so subsequent events this frame see the new state.
            logic_state.0 = to_name.clone();

            // Mark handled intent slot for this event (so action_bar doesn't also fire its built-in).
            if let Some(slot_key) = super::action_substitution::intent_slot_key(event_name) {
                handled_intents.0.insert(slot_key);
            }
        }
    }

    // Debug: log when an event has no matching bindings (helps diagnose "why didn't my event fire")
    if !events.is_empty() && !any_intent_matched {
        debug!("No FSM binding matched events: {:?}", events);
    }
}
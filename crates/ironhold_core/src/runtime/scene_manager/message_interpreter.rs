use bevy::prelude::*;
use crate::runtime::messages::*;
use crate::runtime::actions::ActionQueue;
use crate::schema::Action;
use crate::capabilities::action_bar::{CurrentTarget, HandledIntentSlots};
use super::{LoadedRules, LoadedStateMachine, LogicState, BehaviorHandle, EntityFsmState, SpawnId};

pub fn message_interpreter_system(
    mut ui_events: MessageReader<UiEvent>,
    mut game_events: MessageReader<GameEvent>,
    mut scene_events: MessageReader<SceneEvent>,
    mut action_queue: ResMut<ActionQueue>,
    loaded_rules: Res<LoadedRules>,
    logic_state: Res<LogicState>,
    current_target: Res<CurrentTarget>,
    mut handled_intents: ResMut<HandledIntentSlots>,
) {
    let target_id = current_target.0.as_deref().unwrap_or("");
    for event in ui_events.read() {
        let event_name = match event {
            UiEvent::ButtonPressed(trigger) => format!("ui.button_pressed:{}", trigger),
        };
        match_rules(&event_name, &loaded_rules, &logic_state, &mut action_queue, target_id);
    }

    for event in game_events.read() {
        let event_name = match event {
            // The trigger name is used as-is; the caller is responsible for namespacing
            // (e.g. "entity.collected:coin_01", "zone.entered:checkpoint_1").
            GameEvent::Trigger(name) => name.clone(),
        };
        let matched = match_rules(&event_name, &loaded_rules, &logic_state, &mut action_queue, target_id);
        if matched {
            if let Some(slot_key) = intent_slot_key(&event_name) {
                handled_intents.0.insert(slot_key);
            }
        }
    }

    for event in scene_events.read() {
        let event_name = match event {
            SceneEvent::Requested(path) => format!("scene.requested:{}", scene_path_stem(path)),
            SceneEvent::Loaded(path)    => format!("scene.loaded:{}",    scene_path_stem(path)),
            SceneEvent::Ready(path)     => format!("scene.ready:{}",     scene_path_stem(path)),
            SceneEvent::Unloading(path) => format!("scene.unloading:{}", scene_path_stem(path)),
        };
        match_rules(&event_name, &loaded_rules, &logic_state, &mut action_queue, target_id);
    }
}

fn match_rules(
    event_name: &str,
    loaded_rules: &LoadedRules,
    logic_state: &LogicState,
    action_queue: &mut ActionQueue,
    target_id: &str,
) -> bool {
    if loaded_rules.0.is_empty() {
        return false;
    }
    let mut matched = false;
    for rule in &loaded_rules.0 {
        let state_matches = match &rule.when {
            None => true,
            Some(required) => required == &logic_state.0,
        };
        if rule.on == event_name && state_matches {
            matched = true;
            for action in &rule.do_actions {
                info!("Rule Matched! Event: {} -> Action: {:?}", event_name, action);
                action_queue.push(rewrite_target(action.clone(), target_id));
            }
        }
    }
    if !matched {
        debug!("No rule matched event {:?} (state: {:?})", event_name, logic_state.0);
    }
    matched
}

/// Extracts the slot key from an intent event name.
/// `"intent.slot.1:player_01"` → `Some("1")`
fn intent_slot_key(event_name: &str) -> Option<String> {
    event_name
        .strip_prefix("intent.slot.")
        .and_then(|s| s.split(':').next())
        .map(|s| s.to_string())
}

fn scene_path_stem(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path)
        .trim_end_matches(".scene.ron")
        .trim_end_matches(".ron")
}

/// Interprets events against per-entity `StateMachineAsset` behaviors.
///
/// For each entity that has loaded a behavior (`BehaviorHandle` + `EntityFsmState`),
/// this system:
/// 1. Reads all events for this frame (same sources as the global interpreters).
/// 2. Substitutes `{self}` in every transition `on` pattern with the entity's spawn ID.
/// 3. Matches events against the entity's FSM (global_on, in-state on, transitions).
/// 4. On a transition: pushes exit then entry actions (with `{self}` rewritten in those
///    too) and advances `EntityFsmState::current`.
///
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
            SceneEvent::Requested(p) => format!("scene.requested:{}", scene_path_stem(p)),
            SceneEvent::Loaded(p)    => format!("scene.loaded:{}",    scene_path_stem(p)),
            SceneEvent::Ready(p)     => format!("scene.ready:{}",     scene_path_stem(p)),
            SceneEvent::Unloading(p) => format!("scene.unloading:{}", scene_path_stem(p)),
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

/// Substitutes `{self}` in action fields that can contain entity references.
/// Called by `entity_fsm_interpreter_system` before pushing actions onto the queue.
pub(crate) fn rewrite_self(action: Action, spawn_id: &str) -> Action {
    match action {
        Action::PlayAnimationOn { target, clip } => Action::PlayAnimationOn {
            target: target.replace("{self}", spawn_id),
            clip,
        },
        Action::EmitEvent(event) => Action::EmitEvent(event.replace("{self}", spawn_id)),
        Action::Despawn(id) => Action::Despawn(id.replace("{self}", spawn_id)),
        Action::SetDespawnTimer { entity, delay_secs } => Action::SetDespawnTimer {
            entity: entity.replace("{self}", spawn_id),
            delay_secs,
        },
        Action::Spawn { prefab, id, position, spawn_point, yaw_deg, at_entity } => Action::Spawn {
            prefab,
            id: id.map(|i| i.replace("{self}", spawn_id)),
            position,
            spawn_point: spawn_point.map(|s| s.replace("{self}", spawn_id)),
            yaw_deg,
            at_entity: at_entity.map(|e| e.replace("{self}", spawn_id)),
        },
        Action::ModifyStat { key, delta } => Action::ModifyStat {
            key: key.replace("{self}", spawn_id),
            delta,
        },
        Action::SetStat { key, value } => Action::SetStat {
            key: key.replace("{self}", spawn_id),
            value,
        },
        Action::ShowDamagePopup { entity, amount } => Action::ShowDamagePopup {
            entity: entity.replace("{self}", spawn_id),
            amount,
        },
        Action::ShowFloatingText { entity, text, offset } => Action::ShowFloatingText {
            entity: entity.replace("{self}", spawn_id),
            text: text.replace("{self}", spawn_id),
            offset,
        },
        Action::SetEntityVisible { entity, visible } => Action::SetEntityVisible {
            entity: entity.replace("{self}", spawn_id),
            visible,
        },
        Action::EmitEventAfterDelay { event, delay_secs } => Action::EmitEventAfterDelay {
            event: event.replace("{self}", spawn_id),
            delay_secs,
        },
        Action::SpawnEffect { key, position, entity } => Action::SpawnEffect {
            key,
            position,
            entity: entity.map(|e| e.replace("{self}", spawn_id)),
        },
        Action::ResetToSpawn(id) => Action::ResetToSpawn(id.replace("{self}", spawn_id)),
        Action::AddItem { entity, item_key, count } =>
            Action::AddItem { entity: entity.replace("{self}", spawn_id), item_key, count },
        Action::RemoveItem { entity, item_key, count } =>
            Action::RemoveItem { entity: entity.replace("{self}", spawn_id), item_key, count },
        Action::TransferItem { from, to, item_key, count } =>
            Action::TransferItem {
                from: from.replace("{self}", spawn_id),
                to: to.replace("{self}", spawn_id),
                item_key,
                count,
            },
        Action::OpenShop(id) => Action::OpenShop(id.replace("{self}", spawn_id)),
        Action::OpenContainer(id) => Action::OpenContainer(id.replace("{self}", spawn_id)),
        other => other,
    }
}

/// Substitutes `{target}` in action fields with the current target's spawn ID.
/// Called by all three interpreter systems before pushing actions onto the queue.
/// If `target_id` is empty (no current target), `{target}` is left as-is and a
/// debug message is logged — the action executor will likely fail gracefully.
pub(crate) fn rewrite_target(action: Action, target_id: &str) -> Action {
    fn s(v: &str, t: &str) -> String { v.replace("{target}", t) }
    match action {
        Action::ModifyStat { key, delta } =>
            Action::ModifyStat { key: s(&key, target_id), delta },
        Action::SetStat { key, value } =>
            Action::SetStat { key: s(&key, target_id), value },
        Action::SpawnEffect { key, position, entity } =>
            Action::SpawnEffect { key, position, entity: entity.map(|e| s(&e, target_id)) },
        Action::ShowDamagePopup { entity, amount } =>
            Action::ShowDamagePopup { entity: s(&entity, target_id), amount },
        Action::ShowFloatingText { entity, text, offset } =>
            Action::ShowFloatingText { entity: s(&entity, target_id), text: text.replace("{target}", target_id), offset },
        Action::SetEntityVisible { entity, visible } =>
            Action::SetEntityVisible { entity: s(&entity, target_id), visible },
        Action::Despawn(id) => Action::Despawn(s(&id, target_id)),
        Action::SetDespawnTimer { entity, delay_secs } =>
            Action::SetDespawnTimer { entity: s(&entity, target_id), delay_secs },
        Action::EmitEvent(ev) => Action::EmitEvent(s(&ev, target_id)),
        Action::EmitEventAfterDelay { event, delay_secs } =>
            Action::EmitEventAfterDelay { event: s(&event, target_id), delay_secs },
        Action::PlayAnimationOn { target, clip } =>
            Action::PlayAnimationOn { target: s(&target, target_id), clip },
        Action::Spawn { prefab, id, position, spawn_point, yaw_deg, at_entity } => Action::Spawn {
            prefab,
            id: id.map(|i| s(&i, target_id)),
            position,
            spawn_point: spawn_point.map(|sp| s(&sp, target_id)),
            yaw_deg,
            at_entity: at_entity.map(|e| s(&e, target_id)),
        },
        // Substitutes {target} in the value so rules can track the current target:
        // SetVariable("target_name", "{target}") → SetVariable("target_name", "orc_01")
        Action::SetVariable(key, value) => Action::SetVariable(key, s(&value, target_id)),
        Action::ResetToSpawn(id) => Action::ResetToSpawn(s(&id, target_id)),
        Action::AddItem { entity, item_key, count } =>
            Action::AddItem { entity: s(&entity, target_id), item_key, count },
        Action::RemoveItem { entity, item_key, count } =>
            Action::RemoveItem { entity: s(&entity, target_id), item_key, count },
        Action::TransferItem { from, to, item_key, count } =>
            Action::TransferItem {
                from: s(&from, target_id),
                to: s(&to, target_id),
                item_key,
                count,
            },
        Action::OpenShop(id) => Action::OpenShop(s(&id, target_id)),
        Action::OpenContainer(id) => Action::OpenContainer(s(&id, target_id)),
        other => other,
    }
}

/// Interprets events against a loaded `StateMachineAsset`, driving state transitions and
/// queuing entry/exit actions.  Runs alongside `message_interpreter_system`; only one of
/// the two will have data to act on for any given project (rules vs. FSM).
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
            SceneEvent::Requested(path) => format!("scene.requested:{}", scene_path_stem(path)),
            SceneEvent::Loaded(path)    => format!("scene.loaded:{}",    scene_path_stem(path)),
            SceneEvent::Ready(path)     => format!("scene.ready:{}",     scene_path_stem(path)),
            SceneEvent::Unloading(path) => format!("scene.unloading:{}", scene_path_stem(path)),
        };
        events.push(name);
    }

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
            intent_matched = true;
        }

        if intent_matched {
            if let Some(slot_key) = intent_slot_key(event_name) {
                handled_intents.0.insert(slot_key);
            }
        }
    }
}

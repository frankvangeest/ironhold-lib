//! Item system S3 (`planning/features/done/item_use.md`): clickable inventory slots and the "default-backed" panel verbs.
//!
//! A shop Buy button emits `ui.button_pressed:buy_item:{key}` and an inventory slot click emits
//! `ui.button_pressed:use_item:{key}`. The engine runs the built-in action (`BuyItem` / `UseItem`) for such a click
//! unless the project's `state_machine.ron` binds that exact event (in `global_on` or any state's `on:`), in which case
//! the designer owns it and the engine never runs the default, in any state. Transitions and entity `.behavior.ron`
//! bindings do not count. The bound set is computed from the loaded state machine, not recorded per frame.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::capabilities::inventory::{InventorySlotMarker, LoadedItemCatalog, PlayerInventory};
use crate::runtime::actions::ActionQueue;
use crate::runtime::messages::UiEvent;
use crate::runtime::scene_manager::LoadedStateMachine;
use crate::schema::Action;

pub const BUY_ITEM_PREFIX: &str = "buy_item:";
pub const USE_ITEM_PREFIX: &str = "use_item:";

/// The default-backed click triggers (`buy_item:*` / `use_item:*`, stored without the `ui.button_pressed:` prefix, so a
/// click lookup allocates nothing) that `state_machine.ron` binds somewhere. Rebuilt when the state machine loads. Lookup-only.
#[derive(Resource, Default)]
pub struct BoundPanelEvents(pub HashSet<String>); // det: lookup-only

impl BoundPanelEvents {
    /// Collects every binding (`global_on` and each state's `on:`) whose event is a default-backed click event.
    pub fn from_state_machine(fsm: &crate::schema::project::StateMachineAsset) -> Self {
        let mut set = HashSet::new(); // det: lookup-only
        for binding in fsm.global_on.iter().chain(fsm.states.iter().flat_map(|s| s.on.iter())) {
            if let Some(trigger) = binding.event.strip_prefix("ui.button_pressed:") {
                if trigger.starts_with(BUY_ITEM_PREFIX) || trigger.starts_with(USE_ITEM_PREFIX) {
                    set.insert(trigger.to_string());
                }
            }
        }
        Self(set)
    }
}

/// Pushes `BuyItem` / `UseItem` for a Buy or inventory-slot click that no `state_machine.ron` binding owns.
/// `GameplaySet::Execute`, after `flush_pending_intent_system` and before the executor: a sanctioned `ActionQueue` pusher.
/// A click on an item with no `use:` block runs no default (the event still reached any rule).
pub fn default_panel_actions_system(
    mut ui_events: MessageReader<UiEvent>,
    loaded_fsm: Res<LoadedStateMachine>,
    mut bound: ResMut<BoundPanelEvents>,
    catalog: Res<LoadedItemCatalog>,
    mut action_queue: ResMut<ActionQueue>,
) {
    if loaded_fsm.is_changed() {
        *bound = loaded_fsm.0.as_ref().map(BoundPanelEvents::from_state_machine).unwrap_or_default();
    }
    // A click that arrived before the state machine loaded would be read here with an empty bound set and again by the
    // interpreter on the next frame (a double action). Not reachable today: the panels only exist after the project has loaded.
    for event in ui_events.read() {
        let UiEvent::ButtonPressed(trigger) = event;
        if let Some(key) = trigger.strip_prefix(BUY_ITEM_PREFIX) {
            if !bound.0.contains(trigger.as_str()) {
                action_queue.push(Action::BuyItem(key.to_string()));
            }
        } else if let Some(key) = trigger.strip_prefix(USE_ITEM_PREFIX) {
            let usable = catalog.0.as_ref().and_then(|c| c.items.get(key)).is_some_and(|d| d.use_def.is_some());
            if usable && !bound.0.contains(trigger.as_str()) {
                action_queue.push(Action::UseItem(key.to_string()));
            }
        }
    }
}

/// Emits `ui.button_pressed:use_item:{item_key}` for the item in the clicked inventory slot: a left click or tap (the
/// `Interaction` edge to `Pressed`, like an action bar slot) or a right click while the slot is hovered (bevy_ui only sets
/// `Pressed` for the left button and touch). Empty slots emit nothing. Keeps running while paused but emits nothing, so the
/// `Interaction` baseline keeps advancing; ignored while the inspector is on.
pub fn inventory_slot_click_system(
    slots: Query<(&InventorySlotMarker, Ref<Interaction>)>,
    player_inv: Res<PlayerInventory>,
    mouse: Res<ButtonInput<MouseButton>>,
    sim: Res<crate::runtime::sim_clock::SimClock>,
    mut ui_events: MessageWriter<UiEvent>,
    #[cfg(feature = "inspector")] inspector_enabled: Option<Res<crate::inspector::InspectorEnabled>>,
) {
    if !sim.is_running() {
        return;
    }
    #[cfg(feature = "inspector")]
    if inspector_enabled.is_some_and(|e| e.0) {
        return;
    }
    let right_pressed = mouse.just_pressed(MouseButton::Right);
    for (slot, interaction) in &slots {
        let left_click = interaction.is_changed() && *interaction == Interaction::Pressed;
        let right_click = right_pressed && *interaction == Interaction::Hovered;
        if !(left_click || right_click) {
            continue;
        }
        let Some(stack) = player_inv.slots.get(slot.slot_index).and_then(|s| s.as_ref()) else { continue };
        ui_events.write(UiEvent::ButtonPressed(format!("{USE_ITEM_PREFIX}{}", stack.item_key)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::project::{FsmEventBinding, FsmState, StateMachineAsset};

    fn binding(event: &str) -> FsmEventBinding {
        FsmEventBinding { event: event.to_string(), do_actions: vec![] }
    }

    #[test]
    fn only_global_and_in_state_bindings_of_panel_clicks_count() {
        let fsm = StateMachineAsset {
            schema_version: 1,
            initial_state: "playing".to_string(),
            states: vec![FsmState {
                name: "playing".to_string(),
                entry_actions: vec![],
                exit_actions: vec![],
                on: vec![binding("ui.button_pressed:buy_item:old_key"), binding("ui.button_pressed:close_shop")],
            }],
            transitions: vec![],
            global_on: vec![binding("ui.button_pressed:use_item:health_potion"), binding("item.used:health_potion")],
        };
        let bound = BoundPanelEvents::from_state_machine(&fsm);
        assert!(bound.0.contains("buy_item:old_key"), "an in-state binding counts");
        assert!(bound.0.contains("use_item:health_potion"), "a global_on binding counts");
        assert!(!bound.0.contains("close_shop"), "only the default-backed clicks are recorded");
        assert!(!bound.0.contains("item.used:health_potion"));
        assert_eq!(bound.0.len(), 2);
    }
}

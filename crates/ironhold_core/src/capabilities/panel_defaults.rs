//! Item system S3 (`planning/features/done/item_use.md`): clickable inventory slots and the "default-backed" panel verbs.
//!
//! A shop Buy button emits `ui.button_pressed:buy_item:{key}` and an inventory slot click emits
//! `ui.button_pressed:use_item:{key}`. The engine runs the built-in action (`BuyItem` / `UseItem`) for such a click
//! unless the project's `state_machine.ron` binds that exact event (in `global_on` or any state's `on:`), in which case
//! the designer owns it and the engine never runs the default, in any state. Transitions and entity `.behavior.ron`
//! bindings do not count. The bound set is computed from the loaded state machine, not recorded per frame.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::capabilities::inventory::{InventorySlotMarker, LoadedInventoryUi, LoadedItemCatalog, PlayerInventory};
use crate::capabilities::item_txn::count_item;
use crate::capabilities::slot_menu::{MenuConfirm, MenuRowKind, SlotMenu, SlotMenuRoot, SlotMenuRow, CONFIRM_ARM_MS};
use crate::runtime::actions::ActionQueue;
use crate::runtime::messages::UiEvent;
use crate::runtime::scene_manager::LoadedStateMachine;
use crate::schema::Action;

pub const BUY_ITEM_PREFIX: &str = "buy_item:";
pub const USE_ITEM_PREFIX: &str = "use_item:";
/// Sell one unit of the item (a bag menu row).
pub const SELL_ITEM_PREFIX: &str = "sell_item:";
/// Sell the clicked stack (the bag menu's "Sell N" row, after its confirm).
pub const SELL_STACK_PREFIX: &str = "sell_stack:";
/// Destroy the clicked stack (the bag menu's Destroy row, after its confirm).
pub const DESTROY_ITEM_PREFIX: &str = "destroy_item:";

/// Every default-backed click trigger prefix. The one list the runtime (`BoundPanelEvents`, the default pusher) and
/// `ironhold validate` (reachability, the "bound in a transition runs both" warning) all read, so they cannot drift.
pub const DEFAULT_BACKED_PREFIXES: [&str; 5] =
    [BUY_ITEM_PREFIX, USE_ITEM_PREFIX, SELL_ITEM_PREFIX, SELL_STACK_PREFIX, DESTROY_ITEM_PREFIX];

/// True when `trigger` (without the `ui.button_pressed:` prefix) is one of the default-backed panel clicks.
pub fn is_default_backed_trigger(trigger: &str) -> bool {
    DEFAULT_BACKED_PREFIXES.iter().any(|p| trigger.starts_with(p))
}

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
                if is_default_backed_trigger(trigger) {
                    set.insert(trigger.to_string());
                }
            }
        }
        Self(set)
    }
}

/// Pushes `BuyItem` / `UseItem` / `SellItem` / `DestroyItem` for a Buy, inventory-slot or bag-menu click that no `state_machine.ron` binding owns.
/// `GameplaySet::Execute`, after `flush_pending_intent_system` and before the executor: a sanctioned `ActionQueue` pusher.
/// A click on an item with no `use:` block runs no default (the event still reached any rule).
pub fn default_panel_actions_system(
    mut ui_events: MessageReader<UiEvent>,
    loaded_fsm: Res<LoadedStateMachine>,
    mut bound: ResMut<BoundPanelEvents>,
    catalog: Res<LoadedItemCatalog>,
    mut inventory_ui: ResMut<LoadedInventoryUi>,
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
        } else if let Some(key) = trigger.strip_prefix(SELL_ITEM_PREFIX) {
            if !bound.0.contains(trigger.as_str()) {
                action_queue.push(Action::SellItem { item_key: key.to_string(), count: 1, all: false });
            }
        } else if let Some(key) = trigger.strip_prefix(SELL_STACK_PREFIX) {
            // Taken even when a rule owns the event, so a stale quantity can never reach a later click.
            let (count, all) = take_menu_quantity(&mut inventory_ui, SELL_STACK_PREFIX, key);
            if !bound.0.contains(trigger.as_str()) {
                action_queue.push(Action::SellItem { item_key: key.to_string(), count, all });
            }
        } else if let Some(key) = trigger.strip_prefix(DESTROY_ITEM_PREFIX) {
            let (count, all) = take_menu_quantity(&mut inventory_ui, DESTROY_ITEM_PREFIX, key);
            if !bound.0.contains(trigger.as_str()) {
                action_queue.push(Action::DestroyItem { item_key: key.to_string(), count, all });
            }
        }
    }
}

/// The quantity the bag menu chose for this verb (`prefix`) and `key` (the clicked stack), taken once. A click with no menu behind it (an authored
/// button, a test) has none, and `sell_stack:` / `destroy_item:` then act on every unit of the item.
fn take_menu_quantity(ui: &mut LoadedInventoryUi, prefix: &str, key: &str) -> (u32, bool) {
    if ui.menu_action.as_ref().is_some_and(|(p, k, _)| *p == prefix && k == key) {
        if let Some((_, _, count)) = ui.menu_action.take() {
            return (count, false);
        }
    }
    (1, true)
}

/// Owns the bag's click menu (item system S4, `capabilities/slot_menu.rs`): opening it, closing it on a click outside, the Yes/No
/// step, and emitting the default-backed event a row stands for. Everything in one system so the click that opens the menu can
/// never also count as an outside click in the same frame.
///
/// * Left click or tap on a non-empty slot: opens the menu for it (again on the open slot: closes it).
/// * Right click on a hovered non-empty slot: emits `use_item:{key}` at once (the shortcut) and closes the menu.
/// * A row of the open menu: Use / Sell emit `use_item:` / `sell_item:` and close; Sell N and Destroy switch to a Yes/No; Yes emits
///   `sell_stack:` / `destroy_item:` (with the clicked stack's quantity in `menu_action`) and closes; No goes back.
/// * A press anywhere else closes it.
///
/// Keeps running while paused but does nothing, so the `Interaction` baselines keep advancing (the menu is closed by
/// `slot_menu_sync_system`); ignored while the inspector is on. Empty slots and cells past the bag emit and open nothing.
pub fn inventory_slot_click_system(
    slots: Query<(&InventorySlotMarker, Ref<Interaction>)>,
    rows: Query<(&SlotMenuRow, Ref<Interaction>)>,
    menu_root: Query<&Interaction, With<SlotMenuRoot>>,
    player_inv: Res<PlayerInventory>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Option<Res<Touches>>,
    real_time: Option<Res<Time<Real>>>,
    sim: Res<crate::runtime::sim_clock::SimClock>,
    mut inventory_ui: ResMut<LoadedInventoryUi>,
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
    let has_clock = real_time.is_some();
    let now_ms = real_time.as_ref().map_or(0, |t| t.elapsed().as_millis() as u64);
    let mut emit = |prefix: &str, key: &str| {
        ui_events.write(UiEvent::ButtonPressed(format!("{prefix}{key}")));
    };

    // 1. A row of the open menu.
    if let Some(menu) = inventory_ui.slot_menu.clone() {
        for (row, interaction) in &rows {
            if !(interaction.is_changed() && *interaction == Interaction::Pressed) {
                continue;
            }
            let key = menu.item_key.as_str();
            match row.0 {
                MenuRowKind::Use => {
                    emit(USE_ITEM_PREFIX, key);
                    inventory_ui.slot_menu = None;
                }
                MenuRowKind::SellOne => {
                    emit(SELL_ITEM_PREFIX, key);
                    inventory_ui.slot_menu = None;
                }
                MenuRowKind::SellStack => {
                    inventory_ui.slot_menu = Some(SlotMenu { confirm: Some(MenuConfirm::SellStack), confirm_since_ms: now_ms, ..menu.clone() })
                }
                MenuRowKind::Destroy => {
                    inventory_ui.slot_menu = Some(SlotMenu { confirm: Some(MenuConfirm::Destroy), confirm_since_ms: now_ms, ..menu.clone() })
                }
                MenuRowKind::Yes => {
                    // The question replaces the verb rows in the same spot, so the second click of a double-click on Destroy would land on
                    // Yes: it only counts once the question has been on screen for a moment.
                    if has_clock && now_ms.saturating_sub(menu.confirm_since_ms) < CONFIRM_ARM_MS {
                        return;
                    }
                    match menu.confirm {
                        Some(MenuConfirm::SellStack) => {
                            emit(SELL_STACK_PREFIX, key);
                            inventory_ui.menu_action = Some((SELL_STACK_PREFIX, menu.item_key.clone(), menu.count));
                        }
                        Some(MenuConfirm::Destroy) => {
                            emit(DESTROY_ITEM_PREFIX, key);
                            inventory_ui.menu_action = Some((DESTROY_ITEM_PREFIX, menu.item_key.clone(), menu.count));
                        }
                        None => {}
                    }
                    inventory_ui.slot_menu = None;
                }
                MenuRowKind::No => inventory_ui.slot_menu = Some(SlotMenu { confirm: None, confirm_since_ms: 0, ..menu.clone() }),
                MenuRowKind::Header | MenuRowKind::NoVerbs | MenuRowKind::Prompt => continue,
            }
            return;
        }
    }

    // 2. A click on a slot: left opens (or closes) the menu, right uses at once.
    let right_pressed = mouse.just_pressed(MouseButton::Right);
    let mut left_click: Option<(usize, String, u32)> = None;
    let mut right_click: Option<String> = None;
    for (slot, interaction) in &slots {
        let left = interaction.is_changed() && *interaction == Interaction::Pressed;
        let right = right_pressed && *interaction == Interaction::Hovered;
        if !(left || right) {
            continue;
        }
        // A cell past the bag's size is drawn faded and does nothing (`slots.get` is already `None` there; this states the rule).
        if slot.slot_index >= player_inv.max_slots {
            continue;
        }
        let Some(stack) = player_inv.slots.get(slot.slot_index).and_then(|s| s.as_ref()) else { continue };
        if left {
            left_click = Some((slot.slot_index, stack.item_key.clone(), stack.count));
        } else {
            right_click = Some(stack.item_key.clone());
        }
    }
    if let Some(key) = right_click {
        emit(USE_ITEM_PREFIX, &key);
        if inventory_ui.slot_menu.is_some() {
            inventory_ui.slot_menu = None;
        }
        return;
    }
    if let Some((slot_index, item_key, count)) = left_click {
        let same_slot = inventory_ui.slot_menu.as_ref().is_some_and(|m| m.slot_index == slot_index);
        inventory_ui.slot_menu = if same_slot {
            None
        } else {
            Some(SlotMenu {
                slot_index,
                total: count_item(&player_inv.slots, &item_key),
                item_key,
                count,
                merchant: inventory_ui.active_merchant_id.clone(),
                confirm: None,
                confirm_since_ms: 0,
            })
        };
        return;
    }

    // 3. A press anywhere else closes an open menu.
    if inventory_ui.slot_menu.is_some() {
        let pressed = mouse.just_pressed(MouseButton::Left) || touches.as_ref().is_some_and(|t| t.any_just_pressed());
        let over_menu = menu_root.iter().any(|i| *i != Interaction::None) || rows.iter().any(|(_, i)| *i != Interaction::None);
        if pressed && !over_menu {
            inventory_ui.slot_menu = None;
        }
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

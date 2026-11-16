//! Item system S6 (`planning/features/done/action_bar_assignable_slots.md`): action bar slots the player fills from the bag.
//!
//! A slot marked `assignable: true` carries [`AssignableSlot`]. What it holds is the item's own [`ActionSlotItem`] (S5), decided by one
//! reconcile: the entry in [`ActionBarAssignments`] if there is one (`Assigned(item)` or `Cleared`), else the slot's authored `item:` default.
//! The resource is never reset by `LoadScene`, so the player's layout survives scene changes (and is lost on a page reload: no save system).
//!
//! The player's gestures (mouse and touch) go through one **assign mode** held in `LoadedInventoryUi.assign_mode` (so a Replace-mode scene
//! load cancels it for free):
//! * the bag menu's **Put on bar** row starts it with a bag item;
//! * with the bag open, a click on a filled assignable slot picks its item up (`from` = that slot), to move it or remove it;
//! * a click on an assignable slot assigns, [Cancel] / a right click / closing the bag / pause cancel.
//!
//! **No race between the bag and the bar.** [`bar_assign_click_system`] (`EmitSet::Ui`, right after `inventory_slot_click_system`) is the one
//! owner of bar-slot clicks while the mode is on; it only *writes* a handoff (`assign_handoff`, `assign_end`) in Emit.
//! `default_panel_actions_system` (Execute) turns the handoff into `AssignBarSlot` / `ClearBarSlot` and ends the mode, even when a rule owns
//! the event. `action_bar_input_system` runs after it and treats a click as consumed while the mode is on, so a click in the mode never
//! uses a slot nor emits `on_cooldown`. Cancels that depend on state (the bag closed, a pause, the item left the bag) are derived every
//! frame by [`bar_assign_sync_system`] (`PostExecute`), never from a list of actions.

use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::capabilities::action_bar::{ActionSlotItem, ActionSlotUi};
use crate::capabilities::inventory::{
    InventoryPanelMarker, LoadedInventoryUi, LoadedItemCatalog, PlayerInventory,
};
use crate::capabilities::item_txn::count_item;
use crate::capabilities::panel_defaults::{ASSIGN_SLOT_PREFIX, CLEAR_SLOT_PREFIX};
use crate::runtime::messages::UiEvent;
use crate::runtime::sim_clock::SimClock;

/// What the player (or an action) did to an assignable slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Assignment {
    /// This item sits on the slot.
    Assigned(String),
    /// Explicitly emptied: an authored default stays off until something is assigned.
    Cleared,
}

/// The player's layout of assignable action bar slots, keyed by the bar's `id` and the slot's `key`. Never reset by `LoadScene`; empty at app start.
/// The map is private behind accessors so a save system can serialise it later.
#[derive(Resource, Default, Debug)]
pub struct ActionBarAssignments(BTreeMap<(String, String), Assignment>);

impl ActionBarAssignments {
    pub fn get(&self, bar: &str, key: &str) -> Option<&Assignment> {
        self.0.get(&(bar.to_string(), key.to_string()))
    }
    pub fn assign(&mut self, bar: &str, key: &str, item: &str) {
        self.0.insert((bar.to_string(), key.to_string()), Assignment::Assigned(item.to_string()));
    }
    pub fn clear(&mut self, bar: &str, key: &str) {
        self.0.insert((bar.to_string(), key.to_string()), Assignment::Cleared);
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// An action bar slot the player can fill. `default` is the slot's authored `item:`.
#[derive(Component, Debug, Clone)]
pub struct AssignableSlot {
    pub bar: String,
    pub key: String,
    pub default: Option<String>,
}

/// The item an assignable slot should hold: the resource's entry if there is one, else the authored default.
pub fn desired_item<'a>(assignments: &'a ActionBarAssignments, slot: &'a AssignableSlot) -> Option<&'a str> {
    match assignments.get(&slot.bar, &slot.key) {
        Some(Assignment::Assigned(item)) => Some(item.as_str()),
        Some(Assignment::Cleared) => None,
        None => slot.default.as_deref(),
    }
}

/// Assign mode: `item` is waiting for a click on an assignable slot. `from` is the slot (bar, key) it was picked up from, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignMode {
    pub item: String,
    pub from: Option<(String, String)>,
}

/// A gesture waiting for `default_panel_actions_system` (Execute) to turn it into an action. The UI event carries only the slot key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignHandoff {
    Assign { bar: String, key: String, item: String, from: Option<(String, String)> },
    Clear { bar: String, key: String },
}

/// The strip inside the bag panel that is shown while assign mode is on.
#[derive(Component)]
pub struct AssignStripRoot;

/// The strip's text.
#[derive(Component)]
pub struct AssignStripText;

/// The strip's buttons.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignStripButton {
    Cancel,
    /// Only shown for an item picked up from a slot.
    Remove,
}

const BORDER_NORMAL: Color = crate::capabilities::action_bar::ACTION_SLOT_BORDER;
const BORDER_HIGHLIGHT: Color = Color::srgba(0.25, 0.95, 0.45, 1.0);
const STRIP_BG: Color = Color::srgba(0.10, 0.18, 0.12, 0.95);
const BUTTON_BG: Color = Color::srgba(0.18, 0.18, 0.24, 0.95);
const TEXT: Color = Color::srgba(0.92, 0.95, 0.92, 1.0);

/// The strip text for an assign mode.
pub fn strip_text(item_name: &str, from_slot: bool) -> String {
    if from_slot {
        format!("Move {item_name}: choose a highlighted slot")
    } else {
        format!("Choose a highlighted slot for {item_name}")
    }
}

/// Spawns the hidden strip as a child of an `InventoryPanel` root (called once per panel at scene load).
pub fn spawn_assign_strip(panel: &mut ChildSpawnerCommands, font_size: f32) {
    let size = (font_size + 2.0).max(12.0);
    panel
        .spawn((
            Name::new("AssignStrip"),
            Node {
                display: Display::None,
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(6.0)),
                width: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(STRIP_BG),
            AssignStripRoot,
        ))
        .with_children(|strip| {
            strip.spawn((
                Name::new("AssignStripText"),
                Text::new(""),
                TextFont { font_size: size, ..default() },
                TextColor(TEXT),
                AssignStripText,
            ));
            for (kind, label) in [(AssignStripButton::Remove, "Remove from bar"), (AssignStripButton::Cancel, "Cancel")] {
                strip
                    .spawn((
                        Name::new(format!("AssignStripButton:{kind:?}")),
                        Button,
                        Node {
                            // Hidden until the strip needs it (Remove only shows for a picked-up item).
                            display: if kind == AssignStripButton::Cancel { Display::Flex } else { Display::None },
                            min_height: Val::Px(40.0),
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(BUTTON_BG),
                        kind,
                    ))
                    .with_children(|b| {
                        b.spawn((Text::new(label), TextFont { font_size: size, ..default() }, TextColor(TEXT)));
                    });
            }
        });
}

/// Whether an item can be put on a bar slot: it has a `use:` block and is not a currency item. The one predicate the executor, the validator's
/// checks and the bag menu's "Put on bar" row share.
pub fn item_can_be_assigned(def: &crate::schema::items::ItemDef) -> bool {
    def.use_def.is_some() && def.currency_stat.is_none()
}

/// The bag panel is on screen (any `InventoryPanel` not hidden).
fn bag_shown(panels: &Query<&Visibility, With<InventoryPanelMarker>>) -> bool {
    panels.iter().any(|v| *v != Visibility::Hidden)
}

/// Owns the bar-slot clicks while the bag is open or assign mode is on (`EmitSet::Ui`, after `inventory_slot_click_system`). Only writes
/// the mode, `assign_handoff`, `assign_end` and `UiEvent`s; never an action.
///
/// * mode on, click on an assignable slot: assign (the same slot it was picked from just cancels);
/// * mode on, click on any other slot: cancel (the click is swallowed: `action_bar_input_system` sees the mode still on this frame);
/// * mode on, [Cancel] / [Remove from bar] on the strip, or a right click anywhere: cancel / remove / cancel;
/// * mode off, bag open, click on a *filled* assignable slot: pick its item up.
///
/// Does nothing while paused or in the inspector; the `Interaction` baselines keep advancing because the system keeps running.
#[allow(clippy::too_many_arguments)]
pub fn bar_assign_click_system(
    slots: Query<(&ActionSlotUi, Option<&AssignableSlot>, Option<&ActionSlotItem>, Ref<Interaction>)>,
    strip: Query<(&AssignStripButton, Ref<Interaction>)>,
    panels: Query<&Visibility, With<InventoryPanelMarker>>,
    mouse: Res<ButtonInput<MouseButton>>,
    sim: Res<SimClock>,
    mut ui: ResMut<LoadedInventoryUi>,
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
    // Idle frames do nothing (and clone nothing): only a press, an edge on a slot or a strip button, or a right click matters.
    let edge = |i: &Ref<Interaction>| i.is_changed() && **i == Interaction::Pressed;
    let any_input = mouse.just_pressed(MouseButton::Right)
        || strip.iter().any(|(_, i)| edge(&i))
        || slots.iter().any(|(_, _, _, i)| edge(&i));
    if !any_input {
        return;
    }
    let mode = ui.assign_mode.clone();

    if let Some(mode) = &mode {
        // The strip's buttons.
        for (button, interaction) in &strip {
            if !(interaction.is_changed() && *interaction == Interaction::Pressed) {
                continue;
            }
            match button {
                AssignStripButton::Cancel => ui.assign_end = true,
                AssignStripButton::Remove => {
                    if let Some((bar, key)) = &mode.from {
                        ui_events.write(UiEvent::ButtonPressed(format!("{CLEAR_SLOT_PREFIX}{key}")));
                        ui.assign_handoff.push(AssignHandoff::Clear { bar: bar.clone(), key: key.clone() });
                    }
                    ui.assign_end = true;
                }
            }
            return;
        }
        // A right click anywhere only cancels: it never clears the slot under the cursor.
        if mouse.just_pressed(MouseButton::Right) {
            ui.assign_end = true;
            return;
        }
    }

    for (slot_ui, assignable, item, interaction) in &slots {
        if !(interaction.is_changed() && *interaction == Interaction::Pressed) {
            continue;
        }
        match (&mode, assignable) {
            // Mode on, an assignable slot: assign, unless it is the slot the item came from (puts it back).
            (Some(mode), Some(a)) => {
                if mode.from.as_ref().is_some_and(|(b, k)| *b == a.bar && *k == a.key) {
                    ui.assign_end = true;
                } else {
                    ui_events.write(UiEvent::ButtonPressed(format!("{ASSIGN_SLOT_PREFIX}{}", a.key)));
                    ui.assign_handoff.push(AssignHandoff::Assign {
                        bar: a.bar.clone(),
                        key: a.key.clone(),
                        item: mode.item.clone(),
                        from: mode.from.clone(),
                    });
                    ui.assign_end = true;
                }
            }
            // Mode on, any other slot: cancel and swallow the click.
            (Some(_), None) => ui.assign_end = true,
            // Bag open, a filled assignable slot: pick its item up.
            (None, Some(a)) => {
                if let (Some(item), true) = (item, bag_shown(&panels)) {
                    ui.assign_mode = Some(AssignMode { item: item.0.clone(), from: Some((a.bar.clone(), a.key.clone())) });
                }
            }
            (None, None) => {}
        }
        let _ = slot_ui;
        return;
    }
}

/// Derives the state-dependent cancels and draws the mode (`GameplaySet::PostExecute`): the bag hidden, a pause, no assignable slot in the
/// scene, or (for an item put on the bar from the bag) the item no longer in the bag cancels the mode. The highlight borders and the strip are
/// written only when the mode changes.
#[allow(clippy::too_many_arguments)]
pub fn bar_assign_sync_system(
    mut ui: ResMut<LoadedInventoryUi>,
    player_inv: Res<PlayerInventory>,
    catalog: Res<LoadedItemCatalog>,
    sim: Res<SimClock>,
    panels: Query<&Visibility, With<InventoryPanelMarker>>,
    assignable: Query<(), With<AssignableSlot>>,
    mut slots: Query<&mut BorderColor, With<AssignableSlot>>,
    mut strip_root: Query<&mut Node, (With<AssignStripRoot>, Without<AssignStripButton>)>,
    mut strip_texts: Query<&mut Text, With<AssignStripText>>,
    mut strip_buttons: Query<(&AssignStripButton, &mut Node), Without<AssignStripRoot>>,
    mut last: Local<Option<AssignMode>>,
    new_slots: Query<(), Added<AssignableSlot>>,
) {
    let cancel = ui.assign_mode.as_ref().is_some_and(|mode| {
        let item_left_bag = mode.from.is_none() && count_item(&player_inv.slots, &mode.item) == 0;
        !bag_shown(&panels) || !sim.is_running() || assignable.is_empty() || item_left_bag
    });
    if cancel {
        ui.assign_mode = None;
    }
    // A slot spawned while the mode is on (an overlay) must get the highlight too, so a new slot forces a redraw.
    if *last == ui.assign_mode && new_slots.is_empty() {
        return;
    }
    *last = ui.assign_mode.clone();
    let active = ui.assign_mode.is_some();
    let want_border = if active { BORDER_HIGHLIGHT } else { BORDER_NORMAL };
    for mut border in slots.iter_mut() {
        if border.top != want_border {
            *border = BorderColor::from(want_border);
        }
    }
    for mut node in strip_root.iter_mut() {
        let want = if active { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    let from_slot = ui.assign_mode.as_ref().is_some_and(|m| m.from.is_some());
    for (button, mut node) in strip_buttons.iter_mut() {
        let want = match button {
            AssignStripButton::Cancel => Display::Flex,
            AssignStripButton::Remove => if from_slot { Display::Flex } else { Display::None },
        };
        if node.display != want {
            node.display = want;
        }
    }
    if let Some(mode) = &ui.assign_mode {
        let name = catalog.0.as_ref().and_then(|c| c.items.get(&mode.item)).map(|d| d.display_name.as_str()).unwrap_or(mode.item.as_str());
        let want = strip_text(name, from_slot);
        for mut text in strip_texts.iter_mut() {
            if text.0 != want {
                text.0 = want.clone();
            }
        }
    }
}

/// Makes every assignable slot hold what it should (`GameplaySet::PostExecute`, before `action_bar_item_slot_system`): a reconcile, not a diff.
/// Runs when the assignments changed or a slot was just spawned; the loader already built a new slot correctly, so that case is a no-op.
pub fn apply_bar_assignments_system(
    mut commands: Commands,
    assignments: Res<ActionBarAssignments>,
    added: Query<(), Added<AssignableSlot>>,
    slots: Query<(Entity, &AssignableSlot, Option<&ActionSlotItem>)>,
) {
    if !(assignments.is_changed() || !added.is_empty()) {
        return;
    }
    for (entity, slot, current) in &slots {
        let want = desired_item(&assignments, slot);
        match (want, current) {
            (Some(w), Some(c)) if c.0 == w => {}
            (Some(w), _) => {
                commands.entity(entity).insert(ActionSlotItem(w.to_string()));
            }
            (None, Some(_)) => {
                commands.entity(entity).remove::<ActionSlotItem>();
            }
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(default: Option<&str>) -> AssignableSlot {
        AssignableSlot { bar: "main".to_string(), key: "7".to_string(), default: default.map(str::to_string) }
    }

    #[test]
    fn the_desired_item_is_the_entry_else_the_authored_default() {
        let mut a = ActionBarAssignments::default();
        let s = slot(Some("mana_potion"));
        assert_eq!(desired_item(&a, &s), Some("mana_potion"), "no entry: the default");
        a.assign("main", "7", "health_potion");
        assert_eq!(desired_item(&a, &s), Some("health_potion"), "an assignment replaces the default");
        a.clear("main", "7");
        assert_eq!(desired_item(&a, &s), None, "a cleared default stays off");
    }

    #[test]
    fn assignments_are_per_bar_and_key() {
        let mut a = ActionBarAssignments::default();
        a.assign("p1_bar", "7", "health_potion");
        a.assign("p2_bar", "7", "mana_potion");
        assert_eq!(a.get("p1_bar", "7"), Some(&Assignment::Assigned("health_potion".to_string())));
        assert_eq!(a.get("p2_bar", "7"), Some(&Assignment::Assigned("mana_potion".to_string())));
        assert_eq!(a.get("p1_bar", "8"), None);
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn the_strip_names_the_item_and_whether_it_is_a_move() {
        assert_eq!(strip_text("Health Potion", false), "Choose a highlighted slot for Health Potion");
        assert_eq!(strip_text("Health Potion", true), "Move Health Potion: choose a highlighted slot");
    }
}

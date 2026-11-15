//! Item system S4 (`planning/features/item_sell_destroy.md`): the click menu on a bag slot.
//!
//! A left click or tap on an item in the `InventoryPanel` opens a small menu next to it: **Use** (the item has a `use:`
//! block), **Sell** and **Sell N** (a shop is open and pays for the item), **Destroy**. Sell N and Destroy ask Yes/No first.
//! A row emits a default-backed `ui.button_pressed:*` event (`use_item:`, `sell_item:`, `sell_stack:`, `destroy_item:`), so a
//! rule in `state_machine.ron` can replace any of them; the menu itself never changes the bag. Right click still uses the
//! item at once.
//!
//! Three pieces, each small:
//! * [`menu_view`] is a pure function from the facts about the item to the text of every row (unit-tested here, no Bevy).
//! * `inventory_slot_click_system` (in `panel_defaults.rs`, the one owner of opening, outside-click closing, the confirm step
//!   and emitting, so an opening click can never also count as an outside click) writes [`SlotMenu`] into
//!   `LoadedInventoryUi.slot_menu`.
//! * [`slot_menu_sync_system`] (`GameplaySet::PostExecute`) closes the menu whenever its state went stale (panel hidden, pause,
//!   shop opened or closed, the slot's item or count changed) and draws the rows. Closing is derived from state every frame, not
//!   from a list of actions that could close it, so no action can leave a stale Sell row behind.
//!
//! The menu is one overlay node per `InventoryPanel`, created hidden at scene load and re-parented under the clicked slot, so
//! it needs no cursor or window pixel math (and no `UiScale` correction). Its rows are plain `Button`s with no `UiAction`
//! (`button_system` has no pause gate and the Destroy row must emit nothing). Its nodes carry no `UiNodeId`, so
//! `Action::SetUiVisible` cannot reach them: this file is the only writer of their `Node.display`.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::capabilities::inventory::{
    InventoryPanelMarker, InventorySlotMarker, LoadedInventoryUi, LoadedItemCatalog, PlayerInventory,
};
use crate::capabilities::item_txn::{count_item, sell_price_for};
use crate::runtime::scene_manager::{LoadedPrefabCatalog, PrefabKey, SpawnRegistry};
use crate::runtime::sim_clock::SimClock;

/// What a Yes/No confirm is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuConfirm {
    /// "Destroy N Item?" then `destroy_item:{key}`.
    Destroy,
    /// "Sell N for P gold?" then `sell_stack:{key}`.
    SellStack,
    /// "Sell all T for P gold?" then `sell_all:{key}`: every unit of the item, over all its stacks.
    SellAll,
    /// "Destroy all T?" then `destroy_all:{key}`: every unit of the item, over all its stacks.
    DestroyAll,
}

/// The open menu: which slot, and what the bag looked like when it opened (the menu closes when any of it changes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotMenu {
    pub slot_index: usize,
    pub item_key: String,
    /// Quantity in the clicked slot's stack: what "Sell N" and "Destroy" act on.
    pub count: u32,
    /// Quantity across every stack of the item: only used to notice that the bag changed.
    pub total: u32,
    /// The shop that was open when the menu opened (`None` = no shop), so a shop opening or closing closes the menu.
    pub merchant: Option<String>,
    pub confirm: Option<MenuConfirm>,
    /// Milliseconds on the real clock when the Yes/No appeared (0 while there is none). A Yes pressed within
    /// [`CONFIRM_ARM_MS`] of that is ignored: the confirm replaces the verb rows in the same spot, so the second click of a
    /// double-click on Destroy would otherwise land on Yes.
    pub confirm_since_ms: u64,
}

/// How long after the Yes/No appears before Yes can be pressed.
pub const CONFIRM_ARM_MS: u64 = 350;

/// Root node of the menu overlay. Holds the panel's layout so the menu can open towards the middle of the panel.
#[derive(Component)]
pub struct SlotMenuRoot {
    pub columns: u32,
    pub rows: u32,
    pub slot_size: f32,
}

/// The rows of the menu, top to bottom. Not all are shown at once: [`MenuView::visible`] decides.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuRowKind {
    Header,
    Use,
    SellOne,
    SellStack,
    SellAll,
    Destroy,
    DestroyAll,
    NoVerbs,
    Prompt,
    Yes,
    No,
}

impl MenuRowKind {
    pub const ALL: [MenuRowKind; 11] = [
        MenuRowKind::Header, MenuRowKind::Use, MenuRowKind::SellOne, MenuRowKind::SellStack, MenuRowKind::SellAll,
        MenuRowKind::Destroy, MenuRowKind::DestroyAll, MenuRowKind::NoVerbs, MenuRowKind::Prompt, MenuRowKind::Yes, MenuRowKind::No,
    ];

    /// Rows the player can press (spawned as `Button`s).
    pub fn is_button(self) -> bool {
        matches!(
            self,
            Self::Use | Self::SellOne | Self::SellStack | Self::SellAll | Self::Destroy | Self::DestroyAll | Self::Yes | Self::No
        )
    }
}

/// A row node of the menu.
#[derive(Component)]
pub struct SlotMenuRow(pub MenuRowKind);

/// The `Text` child of a row.
#[derive(Component)]
pub struct SlotMenuRowText(pub MenuRowKind);

/// What the player can do with an item right now, gathered by the caller. Pure data so `menu_view` needs no ECS.
pub struct MenuFacts<'a> {
    pub display_name: &'a str,
    /// The item has a `use:` block.
    pub usable: bool,
    pub destroyable: bool,
    /// What the open shop pays per unit (`None` = no shop open, or it does not buy this item).
    pub unit_price: Option<u32>,
    /// Name of the currency, for the labels ("gold").
    pub currency: &'a str,
}

/// The text of every row and which are shown.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MenuView {
    pub header: String,
    pub use_row: bool,
    pub sell_one: Option<String>,
    pub sell_stack: Option<String>,
    /// "Sell all T": every unit over every stack. Only offered when the bag holds more of the item than the clicked stack.
    pub sell_all: Option<String>,
    pub destroy: Option<String>,
    /// "Destroy all T": as `sell_all`.
    pub destroy_all: Option<String>,
    /// Nothing can be done with the item (a key item): one dimmed line says so.
    pub no_verbs: bool,
    /// Confirm mode: the question, with Yes and No rows; every verb row is hidden.
    pub prompt: Option<String>,
}

impl MenuView {
    pub fn visible(&self, kind: MenuRowKind) -> bool {
        let confirming = self.prompt.is_some();
        match kind {
            MenuRowKind::Header => true,
            MenuRowKind::Use => !confirming && self.use_row,
            MenuRowKind::SellOne => !confirming && self.sell_one.is_some(),
            MenuRowKind::SellStack => !confirming && self.sell_stack.is_some(),
            MenuRowKind::SellAll => !confirming && self.sell_all.is_some(),
            MenuRowKind::Destroy => !confirming && self.destroy.is_some(),
            MenuRowKind::DestroyAll => !confirming && self.destroy_all.is_some(),
            MenuRowKind::NoVerbs => !confirming && self.no_verbs,
            MenuRowKind::Prompt | MenuRowKind::Yes | MenuRowKind::No => confirming,
        }
    }

    pub fn text(&self, kind: MenuRowKind) -> &str {
        match kind {
            MenuRowKind::Header => &self.header,
            MenuRowKind::Use => "Use",
            MenuRowKind::SellOne => self.sell_one.as_deref().unwrap_or(""),
            MenuRowKind::SellStack => self.sell_stack.as_deref().unwrap_or(""),
            MenuRowKind::SellAll => self.sell_all.as_deref().unwrap_or(""),
            MenuRowKind::Destroy => self.destroy.as_deref().unwrap_or(""),
            MenuRowKind::DestroyAll => self.destroy_all.as_deref().unwrap_or(""),
            MenuRowKind::NoVerbs => "Cannot be sold or destroyed",
            MenuRowKind::Prompt => self.prompt.as_deref().unwrap_or(""),
            MenuRowKind::Yes => "Yes",
            MenuRowKind::No => "No",
        }
    }
}

/// Builds the menu text for `count` units in the clicked stack and `total` units over every stack of the item ("Sell all" and
/// "Destroy all" are only offered when `total` is more than `count`). A single unit sells at once; a stack of more offers "Sell 1" and
/// "Sell N" (with a Yes/No); Destroy always asks.
pub fn menu_view(facts: &MenuFacts, count: u32, total: u32, confirm: Option<MenuConfirm>) -> MenuView {
    let name = facts.display_name;
    let cur = facts.currency;
    let total_price = |n: u32| facts.unit_price.map(|p| p as u64 * n as u64);
    let header = if count > 1 { format!("{name} x{count}") } else { name.to_string() };
    let mut view = MenuView { header, ..Default::default() };
    match confirm {
        Some(MenuConfirm::Destroy) => {
            view.prompt = Some(if count > 1 { format!("Destroy {count}?") } else { "Destroy?".to_string() });
        }
        Some(MenuConfirm::SellStack) => {
            let paid = total_price(count).unwrap_or(0);
            view.prompt = Some(format!("Sell {count} for {paid} {cur}?"));
        }
        Some(MenuConfirm::SellAll) => {
            let paid = total_price(total).unwrap_or(0);
            view.prompt = Some(format!("Sell all {total} for {paid} {cur}?"));
        }
        Some(MenuConfirm::DestroyAll) => {
            view.prompt = Some(format!("Destroy all {total}?"));
        }
        None => {
            view.use_row = facts.usable;
            if let Some(p) = facts.unit_price {
                view.sell_one = Some(if count > 1 { format!("Sell 1 ({p} {cur})") } else { format!("Sell ({p} {cur})") });
                if count > 1 {
                    view.sell_stack = Some(format!("Sell {count} ({} {cur})", total_price(count).unwrap_or(0)));
                }
            }
            if facts.destroyable {
                view.destroy = Some(if count > 1 { format!("Destroy {count}") } else { "Destroy".to_string() });
            }
            // More of the item elsewhere in the bag (another stack, or a non-stackable item over several slots): offer all of it.
            if total > count {
                if facts.unit_price.is_some() {
                    view.sell_all = Some(format!("Sell all {total} ({} {cur})", total_price(total).unwrap_or(0)));
                }
                if facts.destroyable {
                    view.destroy_all = Some(format!("Destroy all {total}"));
                }
            }
            view.no_verbs = !view.use_row && view.sell_one.is_none() && view.destroy.is_none();
        }
    }
    view
}

const MENU_BG: Color = Color::srgba(0.07, 0.07, 0.10, 0.97);
const MENU_BORDER: Color = Color::srgba(0.55, 0.55, 0.65, 0.9);
const ROW_BG: Color = Color::srgba(0.18, 0.18, 0.24, 0.95);
const ROW_BG_DANGER: Color = Color::srgba(0.30, 0.10, 0.10, 0.95);
const TEXT: Color = Color::srgba(0.92, 0.92, 0.95, 1.0);
const TEXT_DIM: Color = Color::srgba(0.60, 0.60, 0.66, 1.0);
/// Smallest row height: a finger needs about this much.
const MIN_ROW_HEIGHT: f32 = 40.0;

/// Spawns the hidden menu overlay as a child of an `InventoryPanel` root. Called once per panel at scene load.
pub fn spawn_slot_menu(panel: &mut ChildSpawnerCommands, columns: u32, rows: u32, slot_size: f32, font_size: f32) {
    let text_size = (font_size + 3.0).max(14.0);
    panel
        .spawn((
            Name::new("SlotMenu"),
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(190.0),
                padding: UiRect::all(Val::Px(4.0)),
                row_gap: Val::Px(2.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(MENU_BG),
            BorderColor::from(MENU_BORDER),
            GlobalZIndex(110),
            // Clicks inside the menu are absorbed here (or by a row above it): no click-through to the slots, the world or the bar.
            Interaction::default(),
            FocusPolicy::Block,
            SlotMenuRoot { columns, rows, slot_size },
        ))
        .with_children(|menu| {
            for kind in MenuRowKind::ALL {
                let node = Node {
                    display: Display::None,
                    min_height: Val::Px(if kind.is_button() { MIN_ROW_HEIGHT } else { 24.0 }),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    align_items: AlignItems::Center,
                    ..default()
                };
                let color = match kind {
                    MenuRowKind::Header | MenuRowKind::NoVerbs | MenuRowKind::Prompt => TEXT_DIM,
                    _ => TEXT,
                };
                let mut row = menu.spawn((Name::new(format!("SlotMenuRow:{kind:?}")), node, SlotMenuRow(kind)));
                if kind.is_button() {
                    row.insert((
                        Button,
                        BackgroundColor(if matches!(kind, MenuRowKind::Destroy | MenuRowKind::DestroyAll) { ROW_BG_DANGER } else { ROW_BG }),
                    ));
                }
                row.with_children(|r| {
                    r.spawn((
                        Name::new("SlotMenuRowText"),
                        Text::new(""),
                        TextFont { font_size: text_size, ..default() },
                        TextColor(color),
                        SlotMenuRowText(kind),
                    ));
                });
            }
        });
}

/// Writes `value` only when it differs (change-detection discipline: an unconditional write re-triggers UI layout).
fn set_val(slot: &mut Val, value: Val) {
    if *slot != value {
        *slot = value;
    }
}

/// Closes the menu when its state went stale, and draws it. `GameplaySet::PostExecute`: it sees the bag after this frame's
/// actions, so a sale or a destroy closes it the same frame.
#[allow(clippy::too_many_arguments)]
pub fn slot_menu_sync_system(
    mut commands: Commands,
    mut inventory_ui: ResMut<LoadedInventoryUi>,
    player_inv: Res<PlayerInventory>,
    items: Res<LoadedItemCatalog>,
    sim: Res<SimClock>,
    prefab_catalog: Option<Res<LoadedPrefabCatalog>>,
    registry: Res<SpawnRegistry>,
    prefab_keys: Query<&PrefabKey>,
    panels: Query<&Visibility, With<InventoryPanelMarker>>,
    slots: Query<(Entity, &InventorySlotMarker)>,
    mut root: Query<(Entity, &mut Node, &SlotMenuRoot, Option<&ChildOf>)>,
    mut rows: Query<(&SlotMenuRow, &mut Node), Without<SlotMenuRoot>>,
    mut texts: Query<(&SlotMenuRowText, &mut Text)>,
) {
    fn hide(root: &mut Query<(Entity, &mut Node, &SlotMenuRoot, Option<&ChildOf>)>) {
        for (_, mut node, _, _) in root.iter_mut() {
            if node.display != Display::None {
                node.display = Display::None;
            }
        }
    }
    let Some(menu) = inventory_ui.slot_menu.clone() else {
        // Closed: make sure the overlay is hidden (guarded: nothing is written once it is).
        hide(&mut root);
        return;
    };

    // The facts about the item and what the open shop pays for it (borrowed: nothing here allocates per frame but the labels).
    let def = items.0.as_ref().and_then(|c| c.items.get(&menu.item_key));
    let merchant_def = menu.merchant.as_ref().and_then(|id| {
        let entity = registry.entities.get(id)?;
        let key = prefab_keys.get(*entity).ok()?;
        prefab_catalog.as_deref()?.0.prefabs.get(&key.0)?.merchant.as_ref()
    });
    let unit_price = match (merchant_def, def) {
        (Some(m), Some(d)) => sell_price_for(m, &menu.item_key, d.sell_price),
        _ => None,
    };

    // Derived closing: the bag panel must still be on screen, the game running, the same shop (or none) open, the slot must
    // still hold the same item in the same quantity, and a "Sell N" question needs a shop that still pays for the item.
    let panel_shown = panels.iter().any(|v| *v != Visibility::Hidden);
    let slot_same = player_inv
        .slots
        .get(menu.slot_index)
        .and_then(|s| s.as_ref())
        .is_some_and(|s| s.item_key == menu.item_key && s.count == menu.count);
    let stale = !panel_shown
        || !sim.is_running()
        || menu.slot_index >= player_inv.max_slots
        || inventory_ui.active_merchant_id != menu.merchant
        || !slot_same
        || count_item(&player_inv.slots, &menu.item_key) != menu.total
        || (matches!(menu.confirm, Some(MenuConfirm::SellStack | MenuConfirm::SellAll)) && unit_price.is_none());
    if stale {
        inventory_ui.slot_menu = None;
        hide(&mut root);
        return;
    }

    let facts = MenuFacts {
        display_name: def.map(|d| d.display_name.as_str()).unwrap_or(menu.item_key.as_str()),
        usable: def.is_some_and(|d| d.use_def.is_some()),
        // An item the catalog does not know (or no catalog at all) is refused by `DestroyItem`, so the menu does not offer it.
        destroyable: def.is_some_and(|d| d.destroyable),
        unit_price,
        currency: merchant_def.map_or("", |m| m.currency_stat.as_str()),
    };
    let view = menu_view(&facts, menu.count, menu.total, menu.confirm);

    let slot_entity = slots.iter().find(|(_, m)| m.slot_index == menu.slot_index).map(|(e, _)| e);
    for (entity, mut node, cfg, parent) in root.iter_mut() {
        if let Some(slot) = slot_entity {
            if parent.map(|p| p.parent()) != Some(slot) {
                commands.entity(entity).insert(ChildOf(slot));
            }
        }
        if node.display != Display::Flex {
            node.display = Display::Flex;
        }
        // Open towards the middle of the panel so the menu stays inside it: the right half of the grid opens to the left,
        // the lower half opens upwards. Offset by half a slot so it overlaps the slot's corner, not its icon.
        let columns = cfg.columns.max(1) as usize;
        let (col, row) = (menu.slot_index % columns, menu.slot_index / columns);
        let offset = Val::Px(cfg.slot_size * 0.5);
        if col * 2 >= columns {
            set_val(&mut node.left, Val::Auto);
            set_val(&mut node.right, offset);
        } else {
            set_val(&mut node.left, offset);
            set_val(&mut node.right, Val::Auto);
        }
        if row * 2 >= cfg.rows.max(1) as usize {
            set_val(&mut node.top, Val::Auto);
            set_val(&mut node.bottom, offset);
        } else {
            set_val(&mut node.top, offset);
            set_val(&mut node.bottom, Val::Auto);
        }
    }
    for (row, mut node) in rows.iter_mut() {
        let want = if view.visible(row.0) { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    for (label, mut text) in texts.iter_mut() {
        let want = view.text(label.0);
        if text.0 != want {
            text.0 = want.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts<'a>(usable: bool, destroyable: bool, unit_price: Option<u32>) -> MenuFacts<'a> {
        MenuFacts { display_name: "Health Potion", usable, destroyable, unit_price, currency: "gold" }
    }

    #[test]
    fn a_single_unit_sells_at_once_and_destroy_asks() {
        let v = menu_view(&facts(true, true, Some(5)), 1, 1, None);
        assert_eq!(v.header, "Health Potion");
        assert!(v.use_row);
        assert_eq!(v.sell_one.as_deref(), Some("Sell (5 gold)"));
        assert_eq!(v.sell_stack, None, "no Sell N for a single unit");
        assert_eq!(v.destroy.as_deref(), Some("Destroy"));
        assert!(!v.no_verbs && v.prompt.is_none());
    }

    #[test]
    fn a_stack_offers_one_or_the_clicked_stack_with_its_quantity() {
        let v = menu_view(&facts(true, true, Some(5)), 10, 10, None);
        assert_eq!(v.header, "Health Potion x10");
        assert_eq!(v.sell_one.as_deref(), Some("Sell 1 (5 gold)"));
        assert_eq!(v.sell_stack.as_deref(), Some("Sell 10 (50 gold)"));
        assert_eq!(v.destroy.as_deref(), Some("Destroy 10"));
    }

    #[test]
    fn no_shop_or_an_unbought_item_has_no_sell_rows() {
        let v = menu_view(&facts(true, true, None), 10, 10, None);
        assert!(v.sell_one.is_none() && v.sell_stack.is_none());
        assert!(v.use_row && v.destroy.is_some());
    }

    #[test]
    fn a_key_item_says_so_instead_of_an_empty_menu() {
        let v = menu_view(&facts(false, false, None), 1, 1, None);
        assert!(v.no_verbs);
        assert!(v.visible(MenuRowKind::NoVerbs) && !v.visible(MenuRowKind::Use) && !v.visible(MenuRowKind::Destroy));
        // an item that can only be used is not a "no verbs" item
        assert!(!menu_view(&facts(true, false, None), 1, 1, None).no_verbs);
    }

    #[test]
    fn confirm_mode_shows_only_the_question_with_yes_and_no() {
        let v = menu_view(&facts(true, true, Some(5)), 10, 10, Some(MenuConfirm::Destroy));
        assert_eq!(v.prompt.as_deref(), Some("Destroy 10?"));
        for kind in [MenuRowKind::Use, MenuRowKind::SellOne, MenuRowKind::SellStack, MenuRowKind::Destroy, MenuRowKind::NoVerbs] {
            assert!(!v.visible(kind), "{kind:?} hidden while confirming");
        }
        assert!(v.visible(MenuRowKind::Header) && v.visible(MenuRowKind::Prompt) && v.visible(MenuRowKind::Yes) && v.visible(MenuRowKind::No));
        let v = menu_view(&facts(true, true, Some(5)), 10, 10, Some(MenuConfirm::SellStack));
        assert_eq!(v.prompt.as_deref(), Some("Sell 10 for 50 gold?"));
        let v = menu_view(&facts(true, true, Some(5)), 1, 1, Some(MenuConfirm::Destroy));
        assert_eq!(v.prompt.as_deref(), Some("Destroy?"), "no quantity for a single unit");
    }

    #[test]
    fn a_huge_stack_price_does_not_overflow() {
        let v = menu_view(&facts(false, false, Some(u32::MAX)), 99, 99, None);
        assert_eq!(v.sell_stack.as_deref(), Some(format!("Sell 99 ({} gold)", u32::MAX as u64 * 99).as_str()));
    }

    #[test]
    fn items_spread_over_several_stacks_offer_all_of_them() {
        // 3 non-stackable swords, one per slot: the clicked slot holds 1, the bag holds 3.
        let v = menu_view(&facts(false, true, Some(30)), 1, 3, None);
        assert_eq!(v.sell_one.as_deref(), Some("Sell (30 gold)"));
        assert_eq!(v.sell_all.as_deref(), Some("Sell all 3 (90 gold)"));
        assert_eq!(v.destroy_all.as_deref(), Some("Destroy all 3"));
        // nothing extra when the clicked stack is all there is
        let v = menu_view(&facts(false, true, Some(30)), 3, 3, None);
        assert!(v.sell_all.is_none() && v.destroy_all.is_none());
        // no price: no Sell all; not destroyable: no Destroy all
        let v = menu_view(&facts(false, false, None), 1, 3, None);
        assert!(v.sell_all.is_none() && v.destroy_all.is_none() && v.no_verbs);
    }

    #[test]
    fn the_all_questions_name_the_total() {
        let v = menu_view(&facts(false, true, Some(30)), 1, 3, Some(MenuConfirm::SellAll));
        assert_eq!(v.prompt.as_deref(), Some("Sell all 3 for 90 gold?"));
        let v = menu_view(&facts(false, true, Some(30)), 1, 3, Some(MenuConfirm::DestroyAll));
        assert_eq!(v.prompt.as_deref(), Some("Destroy all 3?"));
        assert!(!v.visible(MenuRowKind::SellAll) && !v.visible(MenuRowKind::DestroyAll) && v.visible(MenuRowKind::Yes));
    }
}

use bevy::prelude::*;
use std::collections::{BTreeSet, HashMap};
use crate::schema::items::{ItemCatalog, ItemStack};

// ─── Resources ────────────────────────────────────────────────────────────────

pub use crate::schema::project::{DEFAULT_PLAYER_SLOTS, MAX_PLAYER_SLOTS};

/// Background of an inventory slot the bag has, and of one past the bag's size (drawn faded: about 35% of the normal alpha).
pub const SLOT_BG: Color = Color::srgba(0.12, 0.12, 0.16, 0.85);
pub const SLOT_BG_DEAD: Color = Color::srgba(0.12, 0.12, 0.16, 0.30);

/// Remaining shop stock per merchant entity (item key -> remaining; `None` = unlimited), seeded from the merchant's
/// `MerchantDef` the first time it is opened or bought from. Runtime state: a new merchant entity (scene load,
/// respawn) starts full, and `LoadScene` clears the map, so a finite `stock_count` refills on scene load, like chests.
/// Keyed by `Entity` (never iterated on a gameplay path, lookups only).
#[derive(Resource, Debug, Default)]
pub struct MerchantStockState(pub std::collections::BTreeMap<Entity, std::collections::BTreeMap<String, Option<u32>>>);

/// Player inventory — persists across scene transitions (like `LoadedStats`).
/// Addressed by `entity: "player"` in inventory actions.
///
/// Invariant: `slots.len() == max_slots` (`new` and `resize` keep it). The faded-cell colour and the click guard read `max_slots`;
/// the icon and count label read `slots`. A future `SetInventorySize` must keep the two equal.
#[derive(Resource, Debug)]
pub struct PlayerInventory {
    pub slots: Vec<Option<ItemStack>>,
    pub max_slots: usize,
    /// Spawn ID of the current player entity (e.g. `"player_01"`).
    /// Set by `Action::Spawn` when a player-tagged prefab is queued.
    /// Used by shop/container actions to show floating feedback text on the player.
    pub player_spawn_id: Option<String>,
}

// Written by hand: the bag has `DEFAULT_PLAYER_SLOTS` slots until the project loader sizes it from `player_inventory`.
impl Default for PlayerInventory {
    fn default() -> Self { Self::new(DEFAULT_PLAYER_SLOTS) }
}

impl PlayerInventory {
    pub fn new(max_slots: usize) -> Self {
        Self { slots: vec![None; max_slots], max_slots, player_spawn_id: None }
    }

    pub fn resize(&mut self, max_slots: usize) {
        self.max_slots = max_slots;
        self.slots.resize(max_slots, None);
    }
}

/// Loaded item catalog for this project (None when no `items_path` set).
#[derive(Resource, Default)]
pub struct LoadedItemCatalog(pub Option<ItemCatalog>);

/// Stores the ECS entities for the InventoryPanel and ShopPanel so actions can
/// toggle their visibility without a query-in-action-executor dance.
/// Also holds all pre-loaded icon atlases keyed by catalog texture key.
#[derive(Resource, Default)]
pub struct LoadedInventoryUi {
    pub inventory_panel: Option<Entity>,
    pub shop_panel: Option<Entity>,
    /// Spawn ID of the merchant whose shop is currently open. Set by `OpenShop`, cleared by `CloseShop`.
    /// Required by `BuyItem` to look up prices and deduct currency.
    pub active_merchant_id: Option<String>,
    /// Catalog key of the panel's default icon sheet (used when an item has no `icon_sheet`).
    pub panel_icon_sheet: Option<String>,
    /// All icon atlases keyed by catalog texture key (panel default + per-item overrides).
    /// Pre-loaded at panel spawn time so no runtime loading is needed on item pickup.
    pub icon_atlases: HashMap<String, (Handle<Image>, Handle<TextureAtlasLayout>)>, // det: lookup-only
    /// The panels that are currently open. A set, not a counter: opening an open panel and closing
    /// a closed one are both no-ops, so the state can never drift (a counter did: a stray Close
    /// re-enabled interact under a visible panel, a doubled Open disabled it until the next
    /// `LoadScene`). Cleared on `LoadScene`. Read through `world_input_blocked()` by
    /// `interactable_system`, `collectible_system` and `tab_targeting_system`.
    pub open_panels: BTreeSet<PanelKind>,
    /// The bag slot menu that is open, if any (item system S4). Lives here so a Replace-mode scene load, which resets this whole
    /// resource, closes it. Written by `inventory_slot_click_system`; closed by `slot_menu_sync_system` when it goes stale.
    pub slot_menu: Option<crate::capabilities::slot_menu::SlotMenu>,
    /// Event prefix (`sell_stack:` / `destroy_item:`), item key and quantity of the stack a menu row just acted on (Yes on
    /// "Sell N" or Destroy). `default_panel_actions_system` takes it the same frame to give `SellItem`/`DestroyItem` the clicked
    /// stack's quantity; a button that fires the same event without a menu has none and acts on every unit. The prefix keeps an
    /// authored `sell_stack:x` press on the same frame from taking a menu's `destroy_item:x` quantity.
    pub menu_action: Option<(&'static str, String, u32)>,
}

/// The UI panels the engine tracks open/closed state for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PanelKind {
    Inventory,
    Shop,
    Container,
}

impl PanelKind {
    /// A modal panel suppresses world input (F-interact, walk-over pickup, Tab targeting) while it is open.
    /// The shop and container panels belong to an interaction, so they are modal. The inventory is not:
    /// the player can open it to check their items and still interact with the world.
    pub fn is_modal(self) -> bool {
        // Exhaustive on purpose: a new panel kind must decide its modality here, or the build fails.
        match self {
            PanelKind::Inventory => false,
            PanelKind::Shop | PanelKind::Container => true,
        }
    }
}

impl LoadedInventoryUi {
    /// Records one panel opening (`open: true`) or closing (`open: false`). Idempotent.
    pub fn set_panel_open(&mut self, kind: PanelKind, open: bool) {
        if open {
            self.open_panels.insert(kind);
        } else {
            self.open_panels.remove(&kind);
        }
    }

    pub fn is_panel_open(&self, kind: PanelKind) -> bool {
        self.open_panels.contains(&kind)
    }

    /// The first open modal panel (in `PanelKind` order), if any.
    pub fn blocking_panel(&self) -> Option<PanelKind> {
        self.open_panels.iter().copied().find(|k| k.is_modal())
    }

    /// True while a modal panel is open: world input must be suppressed.
    pub fn world_input_blocked(&self) -> bool {
        self.blocking_panel().is_some()
    }
}

/// Stores the ECS entity for the ContainerPanel and the currently-active container.
#[derive(Resource, Default)]
pub struct LoadedContainerUi {
    pub container_panel: Option<Entity>,
    /// ECS entity of the container currently shown in the panel. Cleared by `CloseContainer`.
    pub active_container: Option<Entity>,
    /// Catalog key of the panel's default icon sheet.
    pub panel_icon_sheet: Option<String>,
    /// Pre-loaded icon atlases for the container panel.
    pub icon_atlases: HashMap<String, (Handle<Image>, Handle<TextureAtlasLayout>)>, // det: lookup-only
}

// ─── Components ───────────────────────────────────────────────────────────────

/// Entity-attached inventory for containers (chests, crates).
/// Cleared on scene load via `LevelEntity` despawn — does NOT persist.
#[derive(Component, Debug, Default)]
pub struct Inventory {
    pub slots: Vec<Option<ItemStack>>,
    pub max_slots: usize,
}

impl Inventory {
    pub fn new(max_slots: usize) -> Self {
        Self { slots: vec![None; max_slots], max_slots }
    }
}

/// Marks the root entity of an `InventoryPanel` UI node.
#[derive(Component)]
pub struct InventoryPanelMarker {
    pub columns: u32,
    pub rows: u32,
    pub font_size: f32,
}

/// Marks the "Slots: used/total" footer label inside an `InventoryPanel` (left of the gold label).
/// `inventory_ui_system` keeps its text and colour current: `used` counts occupied bag slots (a stack is one slot).
#[derive(Component)]
pub struct SlotCountLabelMarker;

/// Colour of the slot count text normally, and when every slot of the bag is taken (not "nothing more fits": a partial stack can still grow).
pub const SLOT_COUNT_COLOR: Color = Color::srgba(0.85, 0.85, 0.90, 0.90);
pub const SLOT_COUNT_COLOR_FULL: Color = Color::srgba(1.0, 0.60, 0.20, 1.0);

/// The footer text for a bag: `"Slots: 12/20"`.
pub fn slot_count_text(used: usize, total: usize) -> String {
    format!("Slots: {used}/{total}")
}

/// Marks the gold/currency footer label inside an `InventoryPanel`.
/// `inventory_ui_system` updates its text whenever the gold stat or player inventory changes.
#[derive(Component)]
pub struct GoldLabelMarker {
    /// The `LoadedStats` key to display (e.g. `"gold"`).
    pub stat_key: String,
}

/// Marks a single slot container inside an `InventoryPanel`.
#[derive(Component)]
pub struct InventorySlotMarker {
    pub slot_index: usize,
}

/// Marks the stack-count label child inside an `InventoryPanel` slot.
/// Spawned as the last child of each slot so it renders on top of the icon.
#[derive(Component)]
pub struct InventorySlotLabelMarker {
    pub slot_index: usize,
}

/// Marks the icon `ImageNode` child spawned inside a slot when `icon_sheet` is set.
#[derive(Component)]
pub struct InventorySlotIconMarker {
    pub slot_index: usize,
}

/// Marks the root entity of a `ShopPanel` UI node.
#[derive(Component)]
pub struct ShopPanelMarker {
    pub font_size: f32,
}

/// Marks the scrollable entries area inside a `ShopPanel`.
/// `OpenShop` despawns only this entity's children, preserving the header + close button above it.
#[derive(Component, Default)]
pub struct ShopEntriesContainerMarker;

/// Marks the root entity of a `ContainerPanel` UI node.
#[derive(Component)]
pub struct ContainerPanelMarker {
    pub columns: u32,
    pub rows: u32,
    pub font_size: f32,
}

/// Marks a single slot container inside a `ContainerPanel`.
#[derive(Component)]
pub struct ContainerSlotMarker {
    pub slot_index: usize,
}

/// Marks the icon `ImageNode` child spawned inside a container slot.
#[derive(Component)]
pub struct ContainerSlotIconMarker {
    pub slot_index: usize,
}

// ─── Inventory helpers ────────────────────────────────────────────────────────

/// Whether `slots` contains at least one of `item_key`, regardless of count.
pub fn has_item(slots: &[Option<ItemStack>], item_key: &str) -> bool {
    slots.iter().flatten().any(|slot| slot.item_key == item_key)
}

/// Try to add `count` of `item_key` to `slots` (up to `max_slots`).
/// Returns `(actually_added, inventory_full)`.
pub fn add_to_slots(
    slots: &mut Vec<Option<ItemStack>>,
    max_slots: usize,
    item_key: &str,
    mut count: u32,
    catalog: Option<&ItemCatalog>,
) -> (u32, bool) {
    let max_stack = catalog
        .and_then(|c| c.items.get(item_key))
        .map(|d| if d.stackable { d.max_stack } else { 1 })
        .unwrap_or(99);

    let added_start = count;

    // Fill existing stacks first.
    for slot in slots.iter_mut().flatten() {
        if slot.item_key == item_key && slot.count < max_stack {
            let space = max_stack - slot.count;
            let take = count.min(space);
            slot.count += take;
            count -= take;
            if count == 0 { break; }
        }
    }

    // Open new slots.
    while count > 0 {
        if let Some(empty) = slots.iter_mut().find(|s| s.is_none()) {
            let take = count.min(max_stack);
            *empty = Some(ItemStack { item_key: item_key.to_string(), count: take });
            count -= take;
        } else {
            let full = slots.iter().filter(|s| s.is_some()).count() >= max_slots;
            return (added_start - count, full);
        }
    }

    (added_start, false)
}

/// Try to remove `count` of `item_key` from `slots`.
/// Returns the actual number removed.
pub fn remove_from_slots(
    slots: &mut Vec<Option<ItemStack>>,
    item_key: &str,
    mut count: u32,
) -> u32 {
    let start = count;
    for slot in slots.iter_mut().rev() {
        if slot.as_ref().map(|s| s.item_key == item_key).unwrap_or(false) {
            let s = slot.as_mut().unwrap();
            let take = count.min(s.count);
            s.count -= take;
            count -= take;
            if s.count == 0 { *slot = None; }
            if count == 0 { break; }
        }
    }
    start - count
}

/// Update InventoryPanel slot text, icon nodes, and gold footer to reflect current state.
pub fn inventory_ui_system(
    player_inv: Res<PlayerInventory>,
    catalog: Res<LoadedItemCatalog>,
    inv_ui: Res<LoadedInventoryUi>,
    loaded_stats: Res<crate::schema::stats::LoadedStats>,
    mut label_q: Query<(&InventorySlotLabelMarker, &mut Text)>,
    mut icon_q: Query<(&InventorySlotIconMarker, &mut ImageNode, &mut Visibility)>,
    mut slot_bg_q: Query<(&InventorySlotMarker, &mut BackgroundColor), (Without<InventorySlotLabelMarker>, Without<InventorySlotIconMarker>)>,
    mut slot_count_q: Query<(&mut Text, &mut TextColor), (With<SlotCountLabelMarker>, Without<InventorySlotLabelMarker>, Without<GoldLabelMarker>)>,
    mut gold_q: Query<(&GoldLabelMarker, &mut Text), Without<InventorySlotLabelMarker>>,
    players: Query<(&crate::runtime::SpawnId, Option<&crate::capabilities::player::PlayerIndex>), With<crate::capabilities::player::CharacterController>>,
    stat_maps: Query<(&crate::runtime::SpawnId, &crate::schema::stats::StatMap)>,
) {
    // Gold footer: always refresh (guarded write avoids change-detection spam; one entity). Same currency rule as
    // the shop and the action bar (`item_txn::resolve_stat_source`): the primary player's own stat, else the pool.
    if !gold_q.is_empty() {
        let primary = crate::capabilities::item_txn::pick_primary_player(
            players.iter().map(|(sid, idx)| (sid.0.as_str(), idx.map(|i| i.0))),
            player_inv.player_spawn_id.as_deref(),
        );
        let own_stats = primary.and_then(|pid| stat_maps.iter().find(|(sid, _)| sid.0 == pid).map(|(_, sm)| sm));
        for (marker, mut text) in gold_q.iter_mut() {
            let amount = crate::capabilities::item_txn::resolve_stat_source(&marker.stat_key, own_stats, Some(&*loaded_stats)).0;
            let label = format!("Gold: {:.0}", amount);
            if text.0 != label { text.0 = label; }
        }
    }

    // Footer "Slots: used/total": occupied bag slots over the bag's size; orange when the bag is full. Guarded writes.
    if !slot_count_q.is_empty() {
        let total = player_inv.max_slots;
        let used = player_inv.slots.iter().take(total).filter(|s| s.is_some()).count();
        let label = slot_count_text(used, total);
        let color = if used >= total { SLOT_COUNT_COLOR_FULL } else { SLOT_COUNT_COLOR };
        for (mut text, mut text_color) in slot_count_q.iter_mut() {
            if text.0 != label { text.0 = label.clone(); }
            if text_color.0 != color { text_color.0 = color; }
        }
    }

    // Slots past the bag's size are drawn faded (written both ways, guarded, so a future resize needs no change here).
    for (marker, mut bg) in slot_bg_q.iter_mut() {
        let want = if marker.slot_index < player_inv.max_slots { SLOT_BG } else { SLOT_BG_DEAD };
        if bg.0 != want { bg.0 = want; }
    }

    // Update slot stack labels (count/max: "3/10" for stackable items, empty otherwise).
    // No is_changed() guard: BuyItem and action_executor run in the same frame as this system,
    // and if action_executor runs after, the change_tick equals last_run_tick so is_changed()
    // returns false on the next frame — the update would be permanently missed. Guarded writes
    // inside the loops prevent actual change-detection spam to the renderer.
    for (marker, mut text) in label_q.iter_mut() {
        let label = match player_inv.slots.get(marker.slot_index).and_then(|s| s.as_ref()) {
            Some(stack) => {
                let max_stack = catalog.0.as_ref()
                    .and_then(|c| c.items.get(&stack.item_key))
                    .map(|d| if d.stackable { d.max_stack } else { 1 })
                    .unwrap_or(99);
                if max_stack > 1 { format!("{}/{}", stack.count, max_stack) } else { String::new() }
            }
            _ => String::new(),
        };
        if text.0 != label { text.0 = label; }
    }

    // Update icon nodes. When an item has icon_color, apply it as a multiplicative tint
    // (sRGB values — Bevy linearises internally). When no icon_color, tint is white (no change).
    for (marker, mut img_node, mut vis) in icon_q.iter_mut() {
        match player_inv.slots.get(marker.slot_index).and_then(|s| s.as_ref()) {
            Some(stack) => {
                let item_def = catalog.0.as_ref().and_then(|c| c.items.get(&stack.item_key));
                let icon_index = item_def.map(|d| d.icon_index as usize).unwrap_or(0);
                let sheet_key = item_def
                    .and_then(|d| d.icon_sheet.as_deref())
                    .or(inv_ui.panel_icon_sheet.as_deref());
                if let Some(key) = sheet_key {
                    if let Some((tex, layout)) = inv_ui.icon_atlases.get(key) {
                        if img_node.image != *tex { img_node.image = tex.clone(); }
                        if let Some(ta) = img_node.texture_atlas.as_mut() {
                            if ta.layout != *layout { ta.layout = layout.clone(); }
                            if ta.index != icon_index { ta.index = icon_index; }
                        }
                    }
                }
                let tint = match item_def.and_then(|d| d.icon_color) {
                    Some((r, g, b, a)) => Color::srgba(r, g, b, a),
                    None => Color::WHITE,
                };
                if img_node.color != tint { img_node.color = tint; }
                if *vis != Visibility::Inherited { *vis = Visibility::Inherited; }
            }
            None => {
                if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
            }
        }
    }
}

/// Update ContainerPanel slot text and icon nodes to reflect the active container's Inventory.
/// Runs every frame when a container is open; cheap (≤ 9 slots typical).
pub fn container_ui_system(
    container_ui: Res<LoadedContainerUi>,
    catalog: Res<LoadedItemCatalog>,
    inventory_q: Query<&Inventory>,
    mut slot_q: Query<(&ContainerSlotMarker, &mut Text)>,
    mut icon_q: Query<(&ContainerSlotIconMarker, &mut ImageNode, &mut Visibility)>,
) {
    let Some(container_entity) = container_ui.active_container else { return; };
    let Ok(inv) = inventory_q.get(container_entity) else { return; };

    // Update slot count labels. Flat query (not scoped through ContainerPanelMarker's direct
    // `Children`) since the slot entities are grandchildren of the panel (Panel -> SlotGrid ->
    // Slot), not direct children — matching `inventory_ui_system`'s label_q pattern, which never
    // had this bug because it was never routed through a `Children` walk in the first place.
    for (marker, mut text) in slot_q.iter_mut() {
        let label = match inv.slots.get(marker.slot_index).and_then(|s| s.as_ref()) {
            Some(stack) => {
                let max_stack = catalog.0.as_ref()
                    .and_then(|c| c.items.get(&stack.item_key))
                    .map(|d| if d.stackable { d.max_stack } else { 1 })
                    .unwrap_or(99);
                if max_stack > 1 { format!("{}/{}", stack.count, max_stack) } else { String::new() }
            }
            _ => String::new(),
        };
        if text.0 != label { text.0 = label; }
    }

    // Update icon nodes with sRGB tint when item has icon_color; white otherwise.
    for (marker, mut img_node, mut vis) in icon_q.iter_mut() {
        match inv.slots.get(marker.slot_index).and_then(|s| s.as_ref()) {
            Some(stack) => {
                let item_def = catalog.0.as_ref().and_then(|c| c.items.get(&stack.item_key));
                let icon_index = item_def.map(|d| d.icon_index as usize).unwrap_or(0);
                let sheet_key = item_def
                    .and_then(|d| d.icon_sheet.as_deref())
                    .or(container_ui.panel_icon_sheet.as_deref());
                if let Some(key) = sheet_key {
                    if let Some((tex, layout)) = container_ui.icon_atlases.get(key) {
                        if img_node.image != *tex { img_node.image = tex.clone(); }
                        if let Some(ta) = img_node.texture_atlas.as_mut() {
                            if ta.layout != *layout { ta.layout = layout.clone(); }
                            if ta.index != icon_index { ta.index = icon_index; }
                        }
                    }
                }
                let tint = match item_def.and_then(|d| d.icon_color) {
                    Some((r, g, b, a)) => Color::srgba(r, g, b, a),
                    None => Color::WHITE,
                };
                if img_node.color != tint { img_node.color = tint; }
                if *vis != Visibility::Inherited { *vis = Visibility::Inherited; }
            }
            None => {
                if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
            }
        }
    }
}

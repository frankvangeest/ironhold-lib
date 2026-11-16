// Item system S2a (`planning/features/done/item_transaction_primitive.md`): buy, transfer, add (coins) and remove go through
// one all-or-nothing transaction, merchant stock is runtime state, and every outcome has a defined event.
//
// No test covered BuyItem / TransferItem / AddItem / RemoveItem before this file (only TakeAllFromContainer, in
// corpse_loot_interact_tests.rs), so these tests are the regression net for the item actions.
use bevy::prelude::*;
use ironhold_core::capabilities::inventory::{
    add_to_slots, GoldLabelMarker, Inventory, InventoryPanelMarker, InventorySlotMarker, LoadedInventoryUi, LoadedItemCatalog, PanelKind, PlayerInventory,
    ShopEntriesContainerMarker, ShopPanelMarker,
};
use ironhold_core::runtime::scene_manager::{LoadedStateMachine, LogicState};
use ironhold_core::runtime::UiEvent;
use ironhold_core::schema::project::{FsmEventBinding, FsmState, StateMachineAsset};
use ironhold_core::capabilities::player::{CharacterController, PlayerIndex};
use ironhold_core::schema::player::InputMap;
use ironhold_core::runtime::scene_manager::{LoadedPrefabCatalog, PrefabKey, SpawnRegistry};
use ironhold_core::runtime::{ActionQueue, GameEvent, SpawnId};
use ironhold_core::schema::catalog::{MerchantDef, PrefabCatalog, PrefabDef, PrefabKind, ShopEntry};
use ironhold_core::schema::items::{ItemCatalog, ItemDef};
use ironhold_core::schema::stats::{LiveStat, LoadedStats, StatDef, StatMap};
use ironhold_core::capabilities::slot_menu::{spawn_slot_menu, MenuConfirm, MenuRowKind, SlotMenuRow};
use ironhold_core::schema::Action;

mod support;
use support::setup_test_app;

fn item(name: &str, stackable: bool, currency_stat: Option<&str>) -> ItemDef {
    ItemDef {
        display_name: name.to_string(),
        icon_sheet: None,
        icon_index: 0,
        icon_color: None,
        stackable,
        max_stack: 99,
        weight: 1.0,
        tags: Vec::new(),
        currency_stat: currency_stat.map(str::to_string),
        sell_price: None,
        destroyable: true,
        use_def: None,
    }
}

fn catalog() -> ItemCatalog {
    let mut items = std::collections::BTreeMap::new();
    items.insert("old_key".to_string(), item("Old Key", false, None));
    items.insert("sword".to_string(), item("Sword", false, None));
    items.insert("potion".to_string(), item("Potion", true, None));
    items.insert("pelt".to_string(), item("Wolf Pelt", true, None));
    items.insert("gold_coin".to_string(), item("Gold Coin", true, Some("gold")));
    ItemCatalog { schema_version: 1, items }
}

fn gold(current: f32, max: f32) -> LiveStat {
    let mut s = LiveStat::new(StatDef {
        base: 0.0, min: 0.0, max, soft_max: None, regen_rate: 0.0, regen_delay: 0.0, thresholds: Vec::new(),
    });
    s.current = current;
    s
}

fn events(app: &App) -> Vec<String> {
    app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .filter_map(|e| match e {
            GameEvent::Trigger(name) => Some(name.clone()),
            _ => None,
        })
        .collect()
}

/// Pushes the actions, runs one update and returns the events that update emitted. The message buffer is cleared
/// first: it is double-buffered, so without that the previous action's events would show up here too.
fn run(app: &mut App, actions: Vec<Action>) -> Vec<String> {
    app.world_mut().resource_mut::<Messages<GameEvent>>().clear();
    {
        let mut queue = app.world_mut().resource_mut::<ActionQueue>();
        for a in actions {
            queue.push(a);
        }
    }
    app.update();
    events(app)
}

fn has(events: &[String], name: &str) -> bool {
    events.iter().any(|e| e == name)
}

fn bag_count(app: &App, item: &str) -> u32 {
    app.world().resource::<PlayerInventory>().slots.iter().flatten().filter(|s| s.item_key == item).map(|s| s.count).sum()
}

fn global_gold(app: &App) -> f32 {
    app.world().resource::<LoadedStats>().0["gold"].current
}

fn spawn_merchant(app: &mut App, id: &str) -> Entity {
    let e = app.world_mut().spawn((SpawnId(id.to_string()), PrefabKey("merchant".to_string()))).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert(id.to_string(), e);
    e
}

/// A scene with one merchant selling `old_key` (price 50, `stock` remaining), a ShopPanel, the item catalog,
/// `gold` global gold and a player bag of `bag_slots` slots.
fn shop_app(stock: Option<u32>, gold_now: f32, bag_slots: usize) -> App {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(catalog())));
    let mut prefabs = std::collections::BTreeMap::new();
    prefabs.insert("merchant".to_string(), PrefabDef {
        kind: PrefabKind::Prop,
        merchant: Some(MerchantDef {
            stock: vec![ShopEntry { item_key: "old_key".to_string(), buy_price: 50, sell_price: None, stock_count: stock }],
            currency_stat: "gold".to_string(),
        }),
        ..Default::default()
    });
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog { schema_version: 2, prefabs }));
    let panel = app.world_mut().spawn((ShopPanelMarker { font_size: 14.0 }, Visibility::Hidden)).id();
    app.world_mut().spawn((ShopEntriesContainerMarker, ChildOf(panel)));
    app.world_mut().resource_mut::<LoadedStats>().0.insert("gold".to_string(), gold(gold_now, 99_999.0));
    app.world_mut().insert_resource(PlayerInventory::new(bag_slots));
    spawn_merchant(&mut app, "merchant_01");
    app.update();
    let _ = run(&mut app, vec![Action::OpenShop("merchant_01".to_string())]);
    app
}

fn buy(app: &mut App) -> Vec<String> {
    run(app, vec![Action::BuyItem("old_key".to_string())])
}

// ─── BuyItem ──────────────────────────────────────────────────────────────────

#[test]
fn buying_into_a_full_bag_costs_nothing_and_says_so() {
    let mut app = shop_app(Some(1), 100.0, 1);
    add_to_slots(&mut app.world_mut().resource_mut::<PlayerInventory>().slots, 1, "sword", 1, None);

    let ev = buy(&mut app);
    assert!(!has(&ev, "item.bought:old_key"), "a failed buy must not emit the success event: {ev:?}");
    assert!(has(&ev, "shop.no_room:old_key"), "{ev:?}");
    assert!(has(&ev, "inventory.full:player"), "{ev:?}");
    assert_eq!(global_gold(&app), 100.0, "no gold may be spent when the item does not fit");
    assert_eq!(bag_count(&app, "old_key"), 0);

    // The stock was not used up either: free the slot and the same buy works.
    app.world_mut().resource_mut::<PlayerInventory>().slots[0] = None;
    let ev = buy(&mut app);
    assert!(has(&ev, "item.bought:old_key"), "{ev:?}");
    assert_eq!(global_gold(&app), 50.0);
}

#[test]
fn a_one_of_a_kind_item_sells_out() {
    let mut app = shop_app(Some(1), 200.0, 5);
    let first = buy(&mut app);
    assert!(has(&first, "item.bought:old_key"), "{first:?}");

    let second = buy(&mut app);
    assert!(!has(&second, "item.bought:old_key"), "{second:?}");
    assert!(has(&second, "shop.sold_out"), "generic form: {second:?}");
    assert!(has(&second, "shop.sold_out:old_key"), "item form: {second:?}");
    assert_eq!(bag_count(&app, "old_key"), 1, "only one key was ever for sale");
    assert_eq!(global_gold(&app), 150.0, "the failed second buy must not take gold");
}

#[test]
fn a_sold_out_item_reports_sold_out_even_when_the_player_is_broke() {
    let mut app = shop_app(Some(0), 0.0, 5);
    let ev = buy(&mut app);
    assert!(has(&ev, "shop.sold_out:old_key"), "{ev:?}");
    assert!(!has(&ev, "shop.insufficient_funds"), "sold out wins over insufficient funds: {ev:?}");
}

#[test]
fn not_enough_gold_emits_both_forms_and_changes_nothing() {
    let mut app = shop_app(Some(1), 10.0, 5);
    let ev = buy(&mut app);
    assert!(has(&ev, "shop.insufficient_funds"), "{ev:?}");
    assert!(has(&ev, "shop.insufficient_funds:old_key"), "{ev:?}");
    assert!(!has(&ev, "item.bought:old_key"));
    assert_eq!(global_gold(&app), 10.0);
    assert_eq!(bag_count(&app, "old_key"), 0);
}

#[test]
fn unlimited_stock_never_runs_out() {
    let mut app = shop_app(None, 500.0, 5);
    for _ in 0..3 {
        let ev = buy(&mut app);
        assert!(has(&ev, "item.bought:old_key"), "{ev:?}");
    }
    assert_eq!(bag_count(&app, "old_key"), 3);
    assert_eq!(global_gold(&app), 350.0);
}

#[test]
fn finite_stock_refills_with_a_new_merchant_entity() {
    let mut app = shop_app(Some(1), 200.0, 5);
    assert!(has(&buy(&mut app), "item.bought:old_key"));
    assert!(has(&buy(&mut app), "shop.sold_out:old_key"));

    // A scene load spawns the merchant again as a new entity: stock is runtime state of the old one.
    let old = app.world().resource::<SpawnRegistry>().entities["merchant_01"];
    app.world_mut().despawn(old);
    app.world_mut().resource_mut::<SpawnRegistry>().entities.remove("merchant_01");
    spawn_merchant(&mut app, "merchant_01");
    run(&mut app, vec![Action::OpenShop("merchant_01".to_string())]);
    let ev = buy(&mut app);
    assert!(has(&ev, "item.bought:old_key"), "a respawned merchant starts with full stock: {ev:?}");
}

#[test]
fn the_last_item_bought_shows_sold_out_in_the_shop_list() {
    let mut app = shop_app(Some(1), 200.0, 5);
    let texts = |app: &mut App| -> Vec<String> {
        app.world_mut().query::<&Text>().iter(app.world()).map(|t| t.0.clone()).collect()
    };
    assert!(texts(&mut app).iter().any(|t| t.contains("[1]")), "before the buy the row shows the remaining count");
    buy(&mut app);
    app.update();
    let after = texts(&mut app);
    assert!(after.iter().any(|t| t.contains("Sold out")), "the row stays, marked sold out: {after:?}");
    assert!(!after.iter().any(|t| t.contains("[1]")));
}

#[test]
fn the_players_own_gold_is_spent_before_the_shared_pool() {
    let mut app = shop_app(Some(1), 0.0, 5);
    // A scene-placed player has no `player_spawn_id` from a dynamic spawn; the fallback id plus its own StatMap.
    app.world_mut().resource_mut::<PlayerInventory>().player_spawn_id = Some("player_01".to_string());
    let mut map = StatMap::default();
    map.0.insert("gold".to_string(), gold(100.0, 99_999.0));
    let player = app.world_mut().spawn((SpawnId("player_01".to_string()), map)).id();

    let ev = buy(&mut app);
    assert!(has(&ev, "item.bought:old_key"), "{ev:?}");
    assert_eq!(app.world().get::<StatMap>(player).unwrap().0["gold"].current, 50.0);
    assert_eq!(global_gold(&app), 0.0, "the shared pool is untouched when the player has its own gold");
}

// ─── TransferItem ─────────────────────────────────────────────────────────────

fn chest(app: &mut App, id: &str, slots: usize, items: &[(&str, u32)]) -> Entity {
    let mut inv = Inventory::new(slots);
    for (key, n) in items {
        add_to_slots(&mut inv.slots, inv.max_slots, key, *n, None);
    }
    let e = app.world_mut().spawn((SpawnId(id.to_string()), inv)).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert(id.to_string(), e);
    e
}

fn chest_count(app: &App, e: Entity, item: &str) -> u32 {
    app.world().get::<Inventory>(e).unwrap().slots.iter().flatten().filter(|s| s.item_key == item).map(|s| s.count).sum()
}

#[test]
fn a_transfer_into_a_nearly_full_bag_moves_what_fits_and_keeps_the_rest() {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(catalog())));
    // Bag: 2 slots, one already used, so exactly one non-stackable sword fits.
    let mut bag = PlayerInventory::new(2);
    add_to_slots(&mut bag.slots, 2, "potion", 1, None);
    app.world_mut().insert_resource(bag);
    let c = chest(&mut app, "chest_01", 4, &[("sword", 3)]);
    app.update();

    let ev = run(&mut app, vec![Action::TransferItem {
        from: "chest_01".to_string(), to: "player".to_string(), item_key: "sword".to_string(), count: 3,
    }]);
    assert_eq!(bag_count(&app, "sword"), 1, "{ev:?}");
    assert_eq!(chest_count(&app, c, "sword"), 2, "the overflow stays in the source instead of being destroyed");
    assert!(has(&ev, "inventory.transferred:chest_01:player:sword"), "{ev:?}");
    assert!(has(&ev, "inventory.full:player"), "{ev:?}");

    // Now the bag is full: nothing moves, no success event, a failure event.
    let ev = run(&mut app, vec![Action::TransferItem {
        from: "chest_01".to_string(), to: "player".to_string(), item_key: "sword".to_string(), count: 2,
    }]);
    assert_eq!(chest_count(&app, c, "sword"), 2);
    assert!(!has(&ev, "inventory.transferred:chest_01:player:sword"), "{ev:?}");
    assert!(has(&ev, "inventory.transfer_failed:chest_01:player:sword"), "{ev:?}");
    assert!(has(&ev, "inventory.full:player"), "{ev:?}");
}

#[test]
fn a_transfer_between_two_containers_works() {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(catalog())));
    let a = chest(&mut app, "chest_a", 4, &[("potion", 5)]);
    let b = chest(&mut app, "chest_b", 4, &[]);
    app.update();

    let ev = run(&mut app, vec![Action::TransferItem {
        from: "chest_a".to_string(), to: "chest_b".to_string(), item_key: "potion".to_string(), count: 3,
    }]);
    assert_eq!((chest_count(&app, a, "potion"), chest_count(&app, b, "potion")), (2, 3));
    assert!(has(&ev, "inventory.transferred:chest_a:chest_b:potion"), "{ev:?}");
    assert!(!has(&ev, "inventory.full:chest_b"));
}

#[test]
fn transferring_something_the_source_does_not_hold_fails_cleanly() {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(catalog())));
    let a = chest(&mut app, "chest_a", 4, &[]);
    chest(&mut app, "chest_b", 4, &[]);
    app.update();
    let ev = run(&mut app, vec![Action::TransferItem {
        from: "chest_a".to_string(), to: "chest_b".to_string(), item_key: "potion".to_string(), count: 1,
    }]);
    assert!(has(&ev, "inventory.transfer_failed:chest_a:chest_b:potion"), "{ev:?}");
    assert!(!has(&ev, "inventory.transferred:chest_a:chest_b:potion"));
    assert_eq!(chest_count(&app, a, "potion"), 0);
}

#[test]
fn a_coin_item_transferred_to_the_player_credits_the_gold_stat() {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(catalog())));
    app.world_mut().resource_mut::<LoadedStats>().0.insert("gold".to_string(), gold(5.0, 99_999.0));
    app.world_mut().insert_resource(PlayerInventory::new(1));
    let c = chest(&mut app, "chest_01", 4, &[("gold_coin", 15)]);
    app.update();
    let ev = run(&mut app, vec![Action::TransferItem {
        from: "chest_01".to_string(), to: "player".to_string(), item_key: "gold_coin".to_string(), count: 15,
    }]);
    assert_eq!(global_gold(&app), 20.0, "{ev:?}");
    assert_eq!(bag_count(&app, "gold_coin"), 0, "coins never occupy a slot");
    assert_eq!(chest_count(&app, c, "gold_coin"), 0);
    assert!(has(&ev, "inventory.added:player:gold_coin:15"), "{ev:?}");
}

// ─── AddItem (coins) ──────────────────────────────────────────────────────────

#[test]
fn adding_a_coin_item_credits_the_stat_and_uses_no_slot() {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(catalog())));
    app.world_mut().resource_mut::<LoadedStats>().0.insert("gold".to_string(), gold(0.0, 99_999.0));
    app.world_mut().insert_resource(PlayerInventory::new(3));
    app.update();
    let ev = run(&mut app, vec![Action::AddItem { entity: "player".to_string(), item_key: "gold_coin".to_string(), count: 40 }]);
    assert_eq!(global_gold(&app), 40.0, "{ev:?}");
    assert_eq!(app.world().resource::<PlayerInventory>().slots.iter().flatten().count(), 0);
    assert!(has(&ev, "inventory.added:player:gold_coin:40"), "{ev:?}");
}

#[test]
fn coins_past_the_stat_max_are_not_credited_beyond_it() {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(catalog())));
    app.world_mut().resource_mut::<LoadedStats>().0.insert("gold".to_string(), gold(90.0, 100.0));
    app.world_mut().insert_resource(PlayerInventory::new(3));
    app.update();
    let ev = run(&mut app, vec![Action::AddItem { entity: "player".to_string(), item_key: "gold_coin".to_string(), count: 40 }]);
    assert_eq!(global_gold(&app), 100.0);
    assert!(has(&ev, "inventory.added:player:gold_coin:10"), "{ev:?}");
    assert!(has(&ev, "inventory.full:player"), "the part that did not fit is reported: {ev:?}");
}

// ─── RemoveItem ───────────────────────────────────────────────────────────────

fn remove(entity: &str, item: &str, count: u32, all_or_nothing: bool) -> Action {
    Action::RemoveItem { entity: entity.to_string(), item_key: item.to_string(), count, all_or_nothing }
}

fn pelt_app(pelts: u32) -> App {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(catalog())));
    let mut bag = PlayerInventory::new(3);
    add_to_slots(&mut bag.slots, 3, "pelt", pelts, None);
    app.world_mut().insert_resource(bag);
    app.update();
    app
}

#[test]
fn remove_item_all_or_nothing_takes_nothing_when_there_are_too_few() {
    let mut app = pelt_app(1);
    let ev = run(&mut app, vec![remove("player", "pelt", 3, true)]);
    assert_eq!(bag_count(&app, "pelt"), 1, "all_or_nothing must not take the one pelt held");
    assert!(!has(&ev, "inventory.removed:player:pelt:1"), "{ev:?}");
    assert!(has(&ev, "inventory.remove_failed:player"), "{ev:?}");
    assert!(has(&ev, "inventory.remove_failed:player:pelt"), "{ev:?}");
}

#[test]
fn remove_item_all_or_nothing_removes_the_full_count_when_held() {
    let mut app = pelt_app(3);
    let ev = run(&mut app, vec![remove("player", "pelt", 3, true)]);
    assert_eq!(bag_count(&app, "pelt"), 0);
    assert!(has(&ev, "inventory.removed:player:pelt:3"), "{ev:?}");
}

#[test]
fn remove_item_by_default_still_removes_what_is_held() {
    let mut app = pelt_app(1);
    let ev = run(&mut app, vec![remove("player", "pelt", 3, false)]);
    assert_eq!(bag_count(&app, "pelt"), 0);
    assert!(has(&ev, "inventory.removed:player:pelt:1"), "today's contract is unchanged: {ev:?}");
}

#[test]
fn removing_something_not_held_emits_remove_failed_not_silence() {
    let mut app = pelt_app(0);
    let ev = run(&mut app, vec![remove("player", "pelt", 1, false)]);
    assert!(has(&ev, "inventory.remove_failed:player:pelt"), "{ev:?}");
    assert!(!ev.iter().any(|e| e.starts_with("inventory.removed:")), "{ev:?}");
}

#[test]
fn remove_item_works_on_a_container_too() {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(catalog())));
    let c = chest(&mut app, "chest_01", 4, &[("potion", 4)]);
    app.update();
    let ev = run(&mut app, vec![remove("chest_01", "potion", 3, true)]);
    assert_eq!(chest_count(&app, c, "potion"), 1);
    assert!(has(&ev, "inventory.removed:chest_01:potion:3"), "{ev:?}");
}

// ─── The real "primary player" path, the footer and the built-in message ─────────────────────────────────────────

const DEMO: &str = "../../assets/projects/3rd_person_game_demo";

fn demo_input_map() -> InputMap {
    let src = std::fs::read_to_string(format!("{DEMO}/prefabs/prefabs.ron")).expect("demo prefabs.ron readable");
    let catalog: PrefabCatalog = ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(&src)
        .expect("demo prefabs.ron parses");
    catalog.prefabs["player_warrior"].components.inputs.clone().expect("player_warrior declares inputs")
}

fn controller() -> CharacterController {
    CharacterController {
        walk_speed: 5.0, run_speed: 8.0, rot_speed: 2.0, inputs: demo_input_map(), is_running: false,
        jump_velocity: 5.94, double_jump_enabled: false, double_jump_velocity: 5.94, jumps_used: 0, max_jumps: 1,
        collider_radius: 0.4, ground_cast_length: 0.3, max_walkable_slope_deg: 45.0, coyote_time_secs: 0.1,
        coyote_ticks_remaining: 0, idle_drag: 0.8, jump_air_grace: 0, jump_liftoff_y: None,
    }
}

/// A real player entity (controller + index + own gold stat). `player_spawn_id` stays unset, as for a scene-placed player.
fn spawn_player(app: &mut App, id: &str, index: u32, own_gold: f32) -> Entity {
    let mut map = StatMap::default();
    map.0.insert("gold".to_string(), gold(own_gold, 99_999.0));
    let e = app
        .world_mut()
        .spawn((SpawnId(id.to_string()), controller(), PlayerIndex(index), map, Transform::default(), GlobalTransform::default()))
        .id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert(id.to_string(), e);
    e
}

fn own_gold(app: &App, e: Entity) -> f32 {
    app.world().get::<StatMap>(e).unwrap().0["gold"].current
}

#[test]
fn the_lowest_player_index_pays_not_the_other_player() {
    let mut app = shop_app(Some(1), 0.0, 5);
    // Spawn the higher index first so entity order cannot decide.
    let p2 = spawn_player(&mut app, "player_02", 1, 100.0);
    let p1 = spawn_player(&mut app, "player_01", 0, 100.0);
    app.update();
    let ev = buy(&mut app);
    assert!(has(&ev, "item.bought:old_key"), "{ev:?}");
    assert_eq!(own_gold(&app, p1), 50.0, "player index 0 is the primary player and pays");
    assert_eq!(own_gold(&app, p2), 100.0);
    assert_eq!(global_gold(&app), 0.0);
}

#[test]
fn the_gold_footer_agrees_with_the_shop_about_whose_gold_it_is() {
    let mut app = shop_app(Some(1), 7.0, 5);
    spawn_player(&mut app, "player_01", 0, 100.0);
    let label = app.world_mut().spawn((GoldLabelMarker { stat_key: "gold".to_string() }, Text::new(""))).id();
    app.update();
    assert_eq!(app.world().get::<Text>(label).unwrap().0, "Gold: 100", "the footer shows the player's own gold, not the pool");
    buy(&mut app);
    assert_eq!(app.world().get::<Text>(label).unwrap().0, "Gold: 50", "and follows what the shop spent");
}

fn popup_texts(app: &mut App) -> Vec<String> {
    app.world_mut().query::<&Text2d>().iter(app.world()).map(|t| t.0.clone()).collect()
}

#[test]
fn the_built_in_messages_reach_a_scene_placed_player() {
    let mut app = shop_app(Some(1), 200.0, 1);
    spawn_player(&mut app, "player_01", 0, 200.0);
    app.update();
    buy(&mut app);
    assert!(popup_texts(&mut app).iter().any(|t| t == "Bought Old Key!"), "{:?}", popup_texts(&mut app));
    buy(&mut app);
    assert!(popup_texts(&mut app).iter().any(|t| t == "Sold out!"), "{:?}", popup_texts(&mut app));
}

#[test]
fn the_bag_is_full_message_is_shown() {
    let mut app = shop_app(Some(1), 200.0, 1);
    spawn_player(&mut app, "player_01", 0, 200.0);
    add_to_slots(&mut app.world_mut().resource_mut::<PlayerInventory>().slots, 1, "sword", 1, None);
    app.update();
    buy(&mut app);
    assert!(popup_texts(&mut app).iter().any(|t| t == "Bag is full!"), "{:?}", popup_texts(&mut app));
}

// ─── Same-batch behaviour ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn two_buys_in_one_frame_cannot_sell_the_last_one_twice() {
    let mut app = shop_app(Some(1), 500.0, 5);
    let ev = run(&mut app, vec![Action::BuyItem("old_key".to_string()), Action::BuyItem("old_key".to_string())]);
    assert_eq!(ev.iter().filter(|e| *e == "item.bought:old_key").count(), 1, "{ev:?}");
    assert!(has(&ev, "shop.sold_out:old_key"), "{ev:?}");
    assert_eq!(bag_count(&app, "old_key"), 1);
    assert_eq!(global_gold(&app), 450.0);
}

#[test]
fn a_close_shop_queued_behind_a_buy_stays_closed() {
    let mut app = shop_app(Some(2), 500.0, 5);
    let ev = run(&mut app, vec![Action::BuyItem("old_key".to_string()), Action::CloseShop]);
    assert!(has(&ev, "item.bought:old_key"), "{ev:?}");
    let ui = app.world().resource::<LoadedInventoryUi>();
    assert!(!ui.is_panel_open(PanelKind::Shop), "the refresh after a buy must not re-open a shop closed in the same batch");
    assert_eq!(ui.active_merchant_id, None);
    assert_eq!(ev.iter().filter(|e| *e == "ui.panel_opened").count(), 0, "no second panel_opened: {ev:?}");
}

// ─── Failure events on the silent paths ───────────────────────────────────────────────────────────────────────────

#[test]
fn removing_from_an_entity_that_does_not_exist_is_a_failure_event() {
    let mut app = pelt_app(1);
    let ev = run(&mut app, vec![remove("no_such_chest", "pelt", 1, true)]);
    assert!(has(&ev, "inventory.remove_failed:no_such_chest"), "{ev:?}");
    assert!(has(&ev, "inventory.remove_failed:no_such_chest:pelt"), "{ev:?}");
}

#[test]
fn transferring_to_an_entity_that_does_not_exist_is_a_failure_event() {
    let mut app = pelt_app(1);
    let ev = run(&mut app, vec![Action::TransferItem {
        from: "player".to_string(), to: "no_such_chest".to_string(), item_key: "pelt".to_string(), count: 1,
    }]);
    assert!(has(&ev, "inventory.transfer_failed:player:no_such_chest:pelt"), "{ev:?}");
    assert_eq!(bag_count(&app, "pelt"), 1, "nothing was taken from the source");
}

#[test]
fn a_transfer_from_an_inventory_to_itself_is_a_failure_event() {
    let mut app = pelt_app(1);
    let ev = run(&mut app, vec![Action::TransferItem {
        from: "player".to_string(), to: "player".to_string(), item_key: "pelt".to_string(), count: 1,
    }]);
    assert!(has(&ev, "inventory.transfer_failed:player:player:pelt"), "{ev:?}");
    assert_eq!(bag_count(&app, "pelt"), 1);
}

#[test]
fn removing_zero_items_is_a_failure_not_a_success() {
    let mut app = pelt_app(2);
    let ev = run(&mut app, vec![remove("player", "pelt", 0, false)]);
    assert!(has(&ev, "inventory.remove_failed:player:pelt"), "{ev:?}");
    assert!(!ev.iter().any(|e| e.starts_with("inventory.removed:")), "{ev:?}");
    assert_eq!(bag_count(&app, "pelt"), 2);
}

#[test]
fn a_zero_price_item_can_be_bought_with_no_currency_stat() {
    let mut app = shop_app(Some(1), 0.0, 5);
    // Re-price the one entry to 0 and drop the currency stat entirely.
    app.world_mut().resource_mut::<LoadedStats>().0.clear();
    {
        let mut cat = app.world_mut().resource_mut::<LoadedPrefabCatalog>();
        cat.0.prefabs.get_mut("merchant").unwrap().merchant.as_mut().unwrap().stock[0].buy_price = 0;
    }
    let ev = buy(&mut app);
    assert!(has(&ev, "item.bought:old_key"), "{ev:?}");
}

// ─── Item system S3: using items (`planning/features/done/item_use.md`) ───────────────────────────────────────────────────

fn use_def(consume: u32, do_actions: Vec<Action>) -> ironhold_core::schema::items::ItemUseDef {
    ironhold_core::schema::items::ItemUseDef { do_actions, consume }
}

fn emit_action(name: &str) -> Action {
    Action::EmitEvent(name.to_string())
}

/// The catalog for the use tests: a potion (consume 1), an elixir (consume 2), a reusable horn (consume 0), an item whose use
/// list contains a `UseItem` (loops), a sword with no `use:`, and an item whose effect needs `{self}`.
fn use_catalog() -> ItemCatalog {
    let mut c = catalog();
    let mut with = |key: &str, stackable: bool, u: ironhold_core::schema::items::ItemUseDef| {
        let mut d = item(key, stackable, None);
        d.use_def = Some(u);
        c.items.insert(key.to_string(), d);
    };
    with("potion", true, use_def(1, vec![emit_action("potion.drunk")]));
    with("elixir", true, use_def(2, vec![emit_action("elixir.drunk")]));
    with("horn", false, use_def(0, vec![emit_action("horn.blown")]));
    with("looping", true, use_def(1, vec![Action::UseItem("looping".to_string()), emit_action("loop.effect")]));
    with("blessing", true, use_def(1, vec![Action::ShowFloatingText {
        entity: "{self}".to_string(), text: "+40 HP".to_string(), offset: None,
    }]));
    c
}

fn use_app(bag: &[(&str, u32)]) -> App {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(use_catalog())));
    let mut inv = PlayerInventory::new(6);
    for (key, n) in bag {
        add_to_slots(&mut inv.slots, 6, key, *n, None);
    }
    app.world_mut().insert_resource(inv);
    app.update();
    app
}

fn use_action(key: &str) -> Action {
    Action::UseItem(key.to_string())
}

fn count_of(events: &[String], name: &str) -> usize {
    events.iter().filter(|e| *e == name).count()
}

#[test]
fn using_a_potion_consumes_one_and_runs_its_actions_once() {
    let mut app = use_app(&[("potion", 3)]);
    let ev = run(&mut app, vec![use_action("potion")]);
    assert_eq!(bag_count(&app, "potion"), 2, "{ev:?}");
    assert!(has(&ev, "item.used:potion"), "{ev:?}");
    assert_eq!(count_of(&ev, "potion.drunk"), 1, "the item's actions run once: {ev:?}");
    assert!(!has(&ev, "item.use_failed"));
}

#[test]
fn using_an_item_not_held_changes_nothing_and_fails() {
    let mut app = use_app(&[("sword", 1)]);
    let ev = run(&mut app, vec![use_action("potion")]);
    assert!(has(&ev, "item.use_failed"), "generic form: {ev:?}");
    assert!(has(&ev, "item.use_failed:potion"), "item form: {ev:?}");
    assert!(!has(&ev, "item.used:potion"));
    assert_eq!(count_of(&ev, "potion.drunk"), 0, "no effect runs on a failed use: {ev:?}");
    assert_eq!(bag_count(&app, "sword"), 1);
}

#[test]
fn an_item_without_a_use_block_fails_and_is_kept() {
    let mut app = use_app(&[("sword", 1)]);
    let ev = run(&mut app, vec![use_action("sword")]);
    assert!(has(&ev, "item.use_failed:sword"), "{ev:?}");
    assert_eq!(bag_count(&app, "sword"), 1);
    let ev = run(&mut app, vec![use_action("no_such_item")]);
    assert!(has(&ev, "item.use_failed:no_such_item"), "an unknown item fails the same way: {ev:?}");
}

#[test]
fn consume_two_needs_two() {
    let mut app = use_app(&[("elixir", 1)]);
    let ev = run(&mut app, vec![use_action("elixir")]);
    assert!(has(&ev, "item.use_failed:elixir"), "{ev:?}");
    assert_eq!(bag_count(&app, "elixir"), 1, "one is not enough and none is taken");
    add_to_slots(&mut app.world_mut().resource_mut::<PlayerInventory>().slots, 6, "elixir", 2, None);
    let ev = run(&mut app, vec![use_action("elixir")]);
    assert!(has(&ev, "item.used:elixir"), "{ev:?}");
    assert_eq!(bag_count(&app, "elixir"), 1, "three held, two consumed");
    assert_eq!(count_of(&ev, "elixir.drunk"), 1);
}

#[test]
fn consume_zero_is_reusable_but_needs_one_held() {
    let mut app = use_app(&[("horn", 1)]);
    for _ in 0..2 {
        let ev = run(&mut app, vec![use_action("horn")]);
        assert!(has(&ev, "item.used:horn"), "{ev:?}");
        assert_eq!(count_of(&ev, "horn.blown"), 1);
        assert_eq!(bag_count(&app, "horn"), 1, "none is taken");
    }
    let mut empty = use_app(&[]);
    let ev = run(&mut empty, vec![use_action("horn")]);
    assert!(has(&ev, "item.use_failed:horn"), "holding none still fails: {ev:?}");
    assert_eq!(count_of(&ev, "horn.blown"), 0);
}

#[test]
fn self_in_an_items_actions_is_the_primary_player() {
    let mut app = use_app(&[("blessing", 1)]);
    spawn_player(&mut app, "player_02", 1, 0.0);
    spawn_player(&mut app, "player_01", 0, 0.0);
    app.update();
    let ev = run(&mut app, vec![use_action("blessing")]);
    assert!(has(&ev, "item.used:blessing"), "{ev:?}");
    assert!(popup_texts(&mut app).iter().any(|t| t == "+40 HP"), "{:?}", popup_texts(&mut app));
}

#[test]
fn self_with_no_player_in_the_scene_fails_before_anything_is_taken() {
    let mut app = use_app(&[("blessing", 1)]);
    let ev = run(&mut app, vec![use_action("blessing")]);
    assert!(has(&ev, "item.use_failed:blessing"), "{ev:?}");
    assert_eq!(bag_count(&app, "blessing"), 1, "the item must not be consumed when its effect cannot run");
}

#[test]
fn an_items_effects_run_right_after_the_use_not_after_the_rest_of_the_batch() {
    let mut app = use_app(&[("potion", 1)]);
    let ev = run(&mut app, vec![use_action("potion"), emit_action("queued.behind")]);
    let drunk = ev.iter().position(|e| e == "potion.drunk").expect("effect ran");
    let behind = ev.iter().position(|e| e == "queued.behind").expect("the queued action ran");
    assert!(drunk < behind, "the effect must run before an action queued behind the use: {ev:?}");
}

#[test]
fn a_use_item_inside_a_use_list_is_skipped_instead_of_looping() {
    let mut app = use_app(&[("looping", 5)]);
    let ev = run(&mut app, vec![use_action("looping")]);
    assert_eq!(bag_count(&app, "looping"), 4, "only the outer use consumed one: {ev:?}");
    assert_eq!(count_of(&ev, "item.used:looping"), 1, "{ev:?}");
    assert_eq!(count_of(&ev, "loop.effect"), 1, "the other effects still run once: {ev:?}");
}

// ---- default-backed panel events -----------------------------------------------------------------------------------

fn send_click(app: &mut App, trigger: &str) {
    app.world_mut().resource_mut::<Messages<UiEvent>>().clear();
    app.world_mut().resource_mut::<Messages<GameEvent>>().clear();
    app.world_mut().resource_mut::<Messages<UiEvent>>().write(UiEvent::ButtonPressed(trigger.to_string()));
    app.update();
}

fn fsm_with(state: &str, binding_event: Option<&str>, in_state: bool) -> LoadedStateMachine {
    let b = binding_event.map(|e| FsmEventBinding { event: e.to_string(), do_actions: vec![emit_action("rule.ran")] });
    let (state_on, global_on) = match (b, in_state) {
        (Some(b), true) => (vec![b], vec![]),
        (Some(b), false) => (vec![], vec![b]),
        (None, _) => (vec![], vec![]),
    };
    LoadedStateMachine(Some(StateMachineAsset {
        schema_version: 1,
        initial_state: state.to_string(),
        states: vec![FsmState { name: "playing".to_string(), entry_actions: vec![], exit_actions: vec![], on: state_on }],
        transitions: vec![],
        global_on,
    }))
}

#[test]
fn a_use_click_with_no_rule_runs_the_built_in_use() {
    let mut app = use_app(&[("potion", 2)]);
    send_click(&mut app, "use_item:potion");
    assert_eq!(bag_count(&app, "potion"), 1);
    assert!(has(&events(&app), "item.used:potion"), "{:?}", events(&app));
}

#[test]
fn a_use_click_on_an_item_with_no_use_block_runs_no_default() {
    let mut app = use_app(&[("sword", 1)]);
    send_click(&mut app, "use_item:sword");
    let ev = events(&app);
    assert!(!has(&ev, "item.use_failed:sword") && !has(&ev, "item.use_failed"), "no failure event for a plain click: {ev:?}");
    assert_eq!(bag_count(&app, "sword"), 1);
}

#[test]
fn a_rule_bound_in_the_current_state_replaces_the_built_in_use() {
    let mut app = use_app(&[("potion", 2)]);
    app.world_mut().insert_resource(fsm_with("playing", Some("ui.button_pressed:use_item:potion"), true));
    app.world_mut().resource_mut::<LogicState>().0 = "playing".to_string();
    app.update();
    send_click(&mut app, "use_item:potion");
    let ev = events(&app);
    assert!(has(&ev, "rule.ran"), "the designer's rule ran: {ev:?}");
    assert!(!has(&ev, "item.used:potion"), "and the built-in use did not: {ev:?}");
    assert_eq!(bag_count(&app, "potion"), 2);
}

#[test]
fn a_rule_bound_in_another_state_still_owns_the_event() {
    // Bound anywhere wins: a rule in `playing` means the engine never runs the default, in any state.
    let mut app = use_app(&[("potion", 2)]);
    app.world_mut().insert_resource(fsm_with("playing", Some("ui.button_pressed:use_item:potion"), true));
    app.world_mut().resource_mut::<LogicState>().0 = "paused".to_string();
    app.update();
    send_click(&mut app, "use_item:potion");
    let ev = events(&app);
    assert!(!has(&ev, "rule.ran"), "the rule is state-scoped and the game is in another state: {ev:?}");
    assert!(!has(&ev, "item.used:potion"), "the built-in must not run either: {ev:?}");
    assert_eq!(bag_count(&app, "potion"), 2);
}

#[test]
fn a_global_rule_also_owns_the_event() {
    let mut app = use_app(&[("potion", 2)]);
    app.world_mut().insert_resource(fsm_with("playing", Some("ui.button_pressed:use_item:potion"), false));
    app.world_mut().resource_mut::<LogicState>().0 = "playing".to_string();
    app.update();
    send_click(&mut app, "use_item:potion");
    let ev = events(&app);
    assert!(has(&ev, "rule.ran") && !has(&ev, "item.used:potion"), "{ev:?}");
}

#[test]
fn a_buy_click_with_no_rule_buys() {
    let mut app = shop_app(Some(1), 200.0, 5);
    send_click(&mut app, "buy_item:old_key");
    assert!(has(&events(&app), "item.bought:old_key"), "{:?}", events(&app));
    assert_eq!(bag_count(&app, "old_key"), 1);
}

#[test]
fn an_existing_buy_rule_wins_so_there_is_no_double_buy() {
    let mut app = shop_app(Some(2), 500.0, 5);
    let rule = FsmEventBinding {
        event: "ui.button_pressed:buy_item:old_key".to_string(),
        do_actions: vec![Action::BuyItem("old_key".to_string())],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(StateMachineAsset {
        schema_version: 1,
        initial_state: "playing".to_string(),
        states: vec![FsmState { name: "playing".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![rule] }],
        transitions: vec![],
        global_on: vec![],
    })));
    app.world_mut().resource_mut::<LogicState>().0 = "playing".to_string();
    app.update();
    send_click(&mut app, "buy_item:old_key");
    assert_eq!(bag_count(&app, "old_key"), 1, "the rule's BuyItem ran once and the default did not add a second");
    assert_eq!(global_gold(&app), 450.0);
}

#[test]
fn a_state_scoped_buy_rule_outside_its_state_does_not_buy() {
    // Today's behaviour for existing projects is kept: the demo's Buy rules live in `playing`; in another state the click does nothing.
    let mut app = shop_app(Some(2), 500.0, 5);
    let rule = FsmEventBinding {
        event: "ui.button_pressed:buy_item:old_key".to_string(),
        do_actions: vec![Action::BuyItem("old_key".to_string())],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(StateMachineAsset {
        schema_version: 1,
        initial_state: "paused".to_string(),
        states: vec![FsmState { name: "playing".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![rule] }],
        transitions: vec![],
        global_on: vec![],
    })));
    app.world_mut().resource_mut::<LogicState>().0 = "paused".to_string();
    app.update();
    send_click(&mut app, "buy_item:old_key");
    assert_eq!(bag_count(&app, "old_key"), 0, "no buy outside the rule's state");
    assert_eq!(global_gold(&app), 500.0);
}

// ---- clickable inventory slots --------------------------------------------------------------------------------------

fn slot(app: &mut App, index: usize) -> Entity {
    app.world_mut().spawn((InventorySlotMarker { slot_index: index }, Button, Interaction::None)).id()
}

#[test]
fn a_right_click_on_a_hovered_slot_uses_its_item() {
    let mut app = use_app(&[("potion", 2)]);
    let s = slot(&mut app, 0);
    *app.world_mut().get_mut::<Interaction>(s).unwrap() = Interaction::Hovered;
    app.update();
    app.world_mut().resource_mut::<Messages<GameEvent>>().clear();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    app.update();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
    assert!(has(&events(&app), "item.used:potion"), "{:?}", events(&app));
    assert_eq!(bag_count(&app, "potion"), 1);
}

#[test]
fn a_click_on_an_empty_slot_does_nothing() {
    let mut app = use_app(&[("potion", 1)]);
    let s = slot(&mut app, 3);
    app.update();
    app.world_mut().resource_mut::<Messages<GameEvent>>().clear();
    *app.world_mut().get_mut::<Interaction>(s).unwrap() = Interaction::Pressed;
    app.update();
    assert!(events(&app).iter().all(|e| !e.starts_with("item.")), "{:?}", events(&app));
    assert_eq!(bag_count(&app, "potion"), 1);
}

#[test]
fn a_slot_click_while_paused_does_nothing() {
    use ironhold_core::runtime::sim_clock::{PauseHold, SimClock};
    let mut app = use_app(&[("potion", 2)]);
    let s = slot(&mut app, 0);
    app.update();
    app.world_mut().resource_mut::<SimClock>().hold(PauseHold::Menu);
    app.update();
    app.world_mut().resource_mut::<Messages<GameEvent>>().clear();
    *app.world_mut().get_mut::<Interaction>(s).unwrap() = Interaction::Pressed;
    app.update();
    assert!(!has(&events(&app), "item.used:potion"), "{:?}", events(&app));
    assert_eq!(bag_count(&app, "potion"), 2);
}

// ---- schema -------------------------------------------------------------------------------------------------------

#[test]
fn the_use_block_parses_with_do_actions_and_consume() {
    let ron_src = r#"(schema_version: 1, items: { "horn": ( display_name: "Horn", use: ( do_actions: [ EmitEvent("x") ], consume: 0 ) ) })"#;
    let cat: ItemCatalog = ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(ron_src)
        .expect("a use: block parses");
    let u = cat.items["horn"].use_def.as_ref().expect("use_def");
    assert_eq!((u.consume, u.do_actions.len()), (0, 1));
}

#[test]
fn a_bad_action_or_the_old_field_name_in_items_ron_is_a_parse_error() {
    // The loader logs the error and proceeds with no item catalog (the whole file), so the parse failure must be real.
    let opts = ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let bad_action = r#"(schema_version: 1, items: { "x": ( display_name: "X", use: ( do_actions: [ NoSuchAction("y") ] ) ) })"#;
    assert!(opts.from_str::<ItemCatalog>(bad_action).is_err());
    let old_name = r#"(schema_version: 1, items: { "x": ( display_name: "X", use: ( actions: [] ) ) })"#;
    assert!(opts.from_str::<ItemCatalog>(old_name).is_err(), "`actions:` is not a field of use:, it is `do_actions:`");
}

#[test]
fn the_demo_items_ron_still_parses() {
    let src = std::fs::read_to_string(format!("{DEMO}/items/items.ron")).expect("demo items.ron readable");
    let cat: ItemCatalog = ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(&src)
        .expect("the demo items.ron parses");
    assert!(cat.validate().is_ok());
}

#[test]
fn two_uses_in_one_batch_each_run_their_effects_in_order() {
    let mut app = use_app(&[("potion", 3)]);
    let ev = run(&mut app, vec![use_action("potion"), use_action("potion")]);
    assert_eq!(bag_count(&app, "potion"), 1, "{ev:?}");
    let order: Vec<&str> = ev.iter().map(String::as_str).filter(|e| *e == "item.used:potion" || *e == "potion.drunk").collect();
    assert_eq!(order, ["item.used:potion", "potion.drunk", "item.used:potion", "potion.drunk"], "use1, effect1, use2, effect2: {ev:?}");
}

#[test]
fn a_bad_use_list_with_target_does_not_stop_the_other_effects() {
    // `{target}` is not available in a use list; it stays unreplaced (and warns) but the use itself still works.
    let mut c = use_catalog();
    let mut d = item("Target Potion", true, None);
    d.use_def = Some(use_def(1, vec![
        Action::ShowFloatingText { entity: "{target}".to_string(), text: "x".to_string(), offset: None },
        emit_action("target.potion.drunk"),
    ]));
    c.items.insert("target_potion".to_string(), d);
    let mut app = use_app(&[("target_potion", 1)]);
    app.world_mut().insert_resource(LoadedItemCatalog(Some(c)));
    let ev = run(&mut app, vec![use_action("target_potion")]);
    assert!(has(&ev, "item.used:target_potion"), "{ev:?}");
    assert!(has(&ev, "target.potion.drunk"), "the other effects still run: {ev:?}");
}

// ---- Item system S2b: the player's bag size comes from the project config (`planning/features/done/player_inventory_slots.md`) --------

fn project_from(ron_text: &str) -> Result<ironhold_core::schema::project::ProjectConfig, ron::error::SpannedError> {
    ron::Options::default().from_str(ron_text)
}

#[test]
fn the_bag_defaults_to_20_slots_everywhere() {
    use ironhold_core::schema::project::{ProjectConfig, DEFAULT_PLAYER_SLOTS};
    // A derived Default would be 0 slots; ~45 tests build `ProjectConfig { ..Default::default() }` and load a project.
    assert_eq!(ProjectConfig::default().player_inventory.max_slots, 20);
    let bag = PlayerInventory::default();
    assert_eq!((bag.max_slots, bag.slots.len(), DEFAULT_PLAYER_SLOTS), (20, 20, 20));
    let cfg = project_from("(schema_version: 3, initial_scene: \"main.scene.ron\")").unwrap();
    assert_eq!(cfg.player_inventory.max_slots, 20, "field omitted");
    let cfg = project_from("(schema_version: 3, initial_scene: \"main.scene.ron\", player_inventory: ())").unwrap();
    assert_eq!(cfg.player_inventory.max_slots, 20, "empty struct");
}

#[test]
fn the_bag_size_parses_and_unknown_fields_are_errors() {
    let cfg = project_from("(schema_version: 3, initial_scene: \"m\", player_inventory: (max_slots: 12))").unwrap();
    assert_eq!(cfg.player_inventory.max_slots, 12);
    assert_eq!(cfg.player_inventory.effective_slots(), 12);
    assert!(project_from("(schema_version: 3, initial_scene: \"m\", player_inventory: (max_slot: 12))").is_err(), "typo is an error");
}

#[test]
fn an_out_of_range_bag_size_is_a_validate_error_and_is_clamped_at_runtime() {
    for (n, used) in [(0usize, 1usize), (300, 256), (1, 1), (256, 256)] {
        let cfg = project_from(&format!("(schema_version: 3, initial_scene: \"m\", player_inventory: (max_slots: {n}))")).unwrap();
        assert_eq!(cfg.player_inventory.effective_slots(), used, "{n}");
        let ok = (1..=256).contains(&n);
        let msg = cfg.validate();
        assert_eq!(msg.is_ok(), ok, "{n}: {msg:?}");
        if !ok {
            assert!(msg.unwrap_err().contains("player_inventory.max_slots"));
        }
    }
}

#[test]
fn a_small_bag_refuses_what_does_not_fit() {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(catalog())));
    app.world_mut().insert_resource(PlayerInventory::new(2));
    app.update();
    let ev = run(&mut app, vec![Action::AddItem { entity: "player".to_string(), item_key: "sword".to_string(), count: 3 }]);
    assert_eq!(bag_count(&app, "sword"), 2, "{ev:?}");
    assert!(has(&ev, "inventory.full:player"), "{ev:?}");
}

#[test]
fn a_default_panel_has_exactly_as_many_cells_as_the_default_bag() {
    // `default_inv_columns() * default_inv_rows()` must equal DEFAULT_PLAYER_SLOTS, or a default panel would trip its own validate error.
    let scene: ironhold_core::schema::scene_v2::GameSceneV2 = ron::Options::default()
        .from_str("(schema_version: 2, name: \"m\", ui: [InventoryPanel((id: \"p\", position: (0.0, 0.0)))])")
        .unwrap();
    let ironhold_core::schema::scene_v2::UiNodeDef::InventoryPanel(p) = &scene.ui[0] else { panic!("not a panel") };
    assert_eq!((p.columns * p.rows) as usize, ironhold_core::capabilities::inventory::DEFAULT_PLAYER_SLOTS);
}

#[test]
fn the_panel_vs_bag_check_reports_both_directions() {
    use ironhold_core::schema::scene_v2::{inventory_panel_bag_issue, InventoryBagIssue};
    assert_eq!(inventory_panel_bag_issue(20, 20), None);
    assert_eq!(inventory_panel_bag_issue(12, 20), Some(InventoryBagIssue::PanelSmaller { hidden: 8 }));
    assert_eq!(inventory_panel_bag_issue(20, 12), Some(InventoryBagIssue::PanelLarger { faded: 8 }));
}

#[test]
fn a_click_on_a_cell_past_the_bag_does_nothing_even_with_a_stale_item_there() {
    use ironhold_core::schema::items::ItemStack;
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(use_catalog())));
    let mut inv = PlayerInventory::new(2);
    // Six entries but a bag of two: an item sitting past `max_slots` must not be usable.
    inv.slots = vec![None, None, None, None, Some(ItemStack { item_key: "potion".to_string(), count: 2 }), None];
    app.world_mut().insert_resource(inv);
    let s = slot(&mut app, 4);
    app.update();
    app.world_mut().resource_mut::<Messages<GameEvent>>().clear();
    *app.world_mut().get_mut::<Interaction>(s).unwrap() = Interaction::Pressed;
    app.update();
    assert!(!has(&events(&app), "item.used:potion"), "{:?}", events(&app));
}

#[test]
fn cells_past_the_bag_are_drawn_faded_and_come_back_with_a_bigger_bag() {
    use ironhold_core::capabilities::inventory::{SLOT_BG, SLOT_BG_DEAD};
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(catalog())));
    app.world_mut().insert_resource(PlayerInventory::new(2));
    let inside = app.world_mut().spawn((InventorySlotMarker { slot_index: 1 }, BackgroundColor(SLOT_BG_DEAD))).id();
    let outside = app.world_mut().spawn((InventorySlotMarker { slot_index: 2 }, BackgroundColor(SLOT_BG))).id();
    app.update();
    assert_eq!(app.world().get::<BackgroundColor>(inside).unwrap().0, SLOT_BG, "inside the bag: normal");
    assert_eq!(app.world().get::<BackgroundColor>(outside).unwrap().0, SLOT_BG_DEAD, "past the bag: faded");
    app.world_mut().insert_resource(PlayerInventory::new(3));
    app.update();
    assert_eq!(app.world().get::<BackgroundColor>(outside).unwrap().0, SLOT_BG, "a bigger bag brings the cell back");
}

#[test]
fn the_footer_shows_used_slots_over_the_bag_size_and_turns_orange_when_full() {
    use ironhold_core::capabilities::inventory::{SlotCountLabelMarker, SLOT_COUNT_COLOR, SLOT_COUNT_COLOR_FULL};
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(catalog())));
    app.world_mut().insert_resource(PlayerInventory::new(3));
    let label = app.world_mut().spawn((SlotCountLabelMarker, Text::new(""), TextColor(SLOT_COUNT_COLOR))).id();
    app.update();
    assert_eq!(app.world().get::<Text>(label).unwrap().0, "Slots: 0/3");
    let _ = run(&mut app, vec![
        Action::AddItem { entity: "player".to_string(), item_key: "sword".to_string(), count: 1 },
        Action::AddItem { entity: "player".to_string(), item_key: "potion".to_string(), count: 40 },
    ]);
    app.update();
    // A stack is one slot: a sword and 40 potions are 2 of 3 slots.
    assert_eq!(app.world().get::<Text>(label).unwrap().0, "Slots: 2/3");
    assert_eq!(app.world().get::<TextColor>(label).unwrap().0, SLOT_COUNT_COLOR);
    let _ = run(&mut app, vec![Action::AddItem { entity: "player".to_string(), item_key: "old_key".to_string(), count: 1 }]);
    app.update();
    assert_eq!(app.world().get::<Text>(label).unwrap().0, "Slots: 3/3");
    assert_eq!(app.world().get::<TextColor>(label).unwrap().0, SLOT_COUNT_COLOR_FULL, "a full bag is orange");
    let _ = run(&mut app, vec![Action::RemoveItem { entity: "player".to_string(), item_key: "sword".to_string(), count: 1, all_or_nothing: false }]);
    app.update();
    assert_eq!(app.world().get::<Text>(label).unwrap().0, "Slots: 2/3");
    assert_eq!(app.world().get::<TextColor>(label).unwrap().0, SLOT_COUNT_COLOR, "no longer full");
}

// ---- item system S4: selling and destroying (`planning/features/done/item_sell_destroy.md`) ------------------------------------

fn priced(name: &str, stackable: bool, max_stack: u32, sell_price: Option<u32>, destroyable: bool) -> ItemDef {
    let mut d = item(name, stackable, None);
    d.max_stack = max_stack;
    d.sell_price = sell_price;
    d.destroyable = destroyable;
    d
}

fn sell_catalog() -> ItemCatalog {
    let mut items = std::collections::BTreeMap::new();
    items.insert("potion".to_string(), priced("Potion", true, 10, Some(5), true));
    items.insert("sword".to_string(), priced("Sword", false, 1, Some(30), true));
    items.insert("pelt".to_string(), priced("Wolf Pelt", true, 99, None, true));
    items.insert("old_key".to_string(), priced("Old Key", false, 1, None, false));
    items.insert("ore".to_string(), priced("Ore", true, 99, Some(8), true));
    items.insert("idol".to_string(), priced("Idol", false, 1, Some(20), true));
    items.insert("gold_coin".to_string(), item("Gold Coin", true, Some("gold")));
    ItemCatalog { schema_version: 1, items }
}

/// A scene with a merchant (currency `gold`), an open shop, and a six-slot bag holding `bag`. The merchant's own prices:
/// `old_key` (buy 50, one in stock, buys it back for 10), `ore` (buy 20, `sell_price: 0` = not bought here), `idol` (buy 200, pays 100).
fn sell_app(gold_now: f32, gold_max: f32, bag: &[(&str, u32)]) -> App {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(LoadedItemCatalog(Some(sell_catalog())));
    let entry = |key: &str, buy: u32, sell: Option<u32>, stock: Option<u32>| ShopEntry {
        item_key: key.to_string(), buy_price: buy, sell_price: sell, stock_count: stock,
    };
    let mut prefabs = std::collections::BTreeMap::new();
    prefabs.insert("merchant".to_string(), PrefabDef {
        kind: PrefabKind::Prop,
        merchant: Some(MerchantDef {
            stock: vec![
                entry("old_key", 50, Some(10), Some(1)),
                entry("ore", 20, Some(0), None),
                entry("idol", 200, Some(100), None),
            ],
            currency_stat: "gold".to_string(),
        }),
        ..Default::default()
    });
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog { schema_version: 2, prefabs }));
    let panel = app.world_mut().spawn((ShopPanelMarker { font_size: 14.0 }, Visibility::Hidden)).id();
    app.world_mut().spawn((ShopEntriesContainerMarker, ChildOf(panel)));
    app.world_mut().resource_mut::<LoadedStats>().0.insert("gold".to_string(), gold(gold_now, gold_max));
    let mut inv = PlayerInventory::new(6);
    for (key, n) in bag {
        add_to_slots(&mut inv.slots, 6, key, *n, Some(&sell_catalog()));
    }
    app.world_mut().insert_resource(inv);
    spawn_merchant(&mut app, "merchant_01");
    app.update();
    let _ = run(&mut app, vec![Action::OpenShop("merchant_01".to_string())]);
    app
}

fn sell(key: &str, count: u32, all: bool) -> Action {
    Action::SellItem { item_key: key.to_string(), count, all }
}

fn destroy(key: &str, count: u32, all: bool) -> Action {
    Action::DestroyItem { item_key: key.to_string(), count, all }
}

#[test]
fn selling_takes_the_items_and_pays_the_price_times_the_count() {
    let mut app = sell_app(100.0, 99_999.0, &[("potion", 7)]);
    let ev = run(&mut app, vec![sell("potion", 3, false)]);
    assert!(has(&ev, "item.sold:potion"), "{ev:?}");
    assert!(!has(&ev, "item.sell_refused"), "{ev:?}");
    assert_eq!(bag_count(&app, "potion"), 4);
    assert_eq!(global_gold(&app), 115.0, "3 x 5");
}

#[test]
fn a_merchant_buys_an_item_it_does_not_stock() {
    // The merchant only stocks old_key, ore and idol; the potion is still bought at the item's own price.
    let mut app = sell_app(0.0, 99_999.0, &[("potion", 1)]);
    let ev = run(&mut app, vec![sell("potion", 1, false)]);
    assert!(has(&ev, "item.sold:potion"), "{ev:?}");
    assert_eq!(global_gold(&app), 5.0);
}

#[test]
fn a_merchants_own_price_beats_the_items_price_and_zero_means_not_bought() {
    let mut app = sell_app(0.0, 99_999.0, &[("idol", 1), ("ore", 4)]);
    let ev = run(&mut app, vec![sell("idol", 1, false)]);
    assert!(has(&ev, "item.sold:idol"), "{ev:?}");
    assert_eq!(global_gold(&app), 100.0, "the ShopEntry price (100) beats the item's (20)");

    let ev = run(&mut app, vec![sell("ore", 1, false)]);
    assert!(has(&ev, "item.sell_refused:ore") && !has(&ev, "item.sold:ore"), "`sell_price: 0` on the entry: not bought here: {ev:?}");
    assert_eq!(bag_count(&app, "ore"), 4);
    assert_eq!(global_gold(&app), 100.0);
}

#[test]
fn an_item_with_no_price_cannot_be_sold() {
    let mut app = sell_app(0.0, 99_999.0, &[("pelt", 2)]);
    for key in ["pelt"] {
        let ev = run(&mut app, vec![sell(key, 1, false)]);
        assert!(has(&ev, "item.sell_refused") && has(&ev, &format!("item.sell_refused:{key}")), "{ev:?}");
        assert!(!has(&ev, &format!("item.sold:{key}")), "{ev:?}");
    }
    assert_eq!(bag_count(&app, "pelt"), 2);
    assert_eq!(global_gold(&app), 0.0);
}

#[test]
fn selling_with_no_shop_open_is_refused_and_changes_nothing() {
    let mut app = sell_app(0.0, 99_999.0, &[("potion", 2)]);
    let _ = run(&mut app, vec![Action::CloseShop]);
    let ev = run(&mut app, vec![sell("potion", 1, false)]);
    assert!(has(&ev, "item.sell_refused:potion") && !has(&ev, "item.sold:potion"), "{ev:?}");
    assert_eq!(bag_count(&app, "potion"), 2);
    assert_eq!(global_gold(&app), 0.0);
}

#[test]
fn selling_more_than_held_or_zero_is_refused_and_changes_nothing() {
    let mut app = sell_app(0.0, 99_999.0, &[("potion", 2)]);
    for (count, all) in [(3, false), (0, false)] {
        let ev = run(&mut app, vec![sell("potion", count, all)]);
        assert!(has(&ev, "item.sell_refused:potion") && !has(&ev, "item.sold:potion"), "count {count}: {ev:?}");
    }
    assert_eq!(bag_count(&app, "potion"), 2, "all or nothing");
    assert_eq!(global_gold(&app), 0.0);
    // `all: true` with none held is refused too
    let mut app = sell_app(0.0, 99_999.0, &[]);
    let ev = run(&mut app, vec![sell("potion", 1, true)]);
    assert!(has(&ev, "item.sell_refused:potion"), "{ev:?}");
}

#[test]
fn sell_all_sells_every_unit_across_stacks() {
    // 25 potions at 10 per stack are three slots.
    let mut app = sell_app(0.0, 99_999.0, &[("potion", 25)]);
    assert_eq!(app.world().resource::<PlayerInventory>().slots.iter().flatten().count(), 3);
    let ev = run(&mut app, vec![sell("potion", 1, true)]);
    assert!(has(&ev, "item.sold:potion"), "{ev:?}");
    assert_eq!(bag_count(&app, "potion"), 0);
    assert_eq!(global_gold(&app), 125.0);
}

#[test]
fn a_payout_past_the_currency_maximum_is_refused_and_keeps_the_items() {
    let mut app = sell_app(95.0, 100.0, &[("potion", 3)]);
    let ev = run(&mut app, vec![sell("potion", 3, false)]);
    assert!(has(&ev, "item.sell_refused:potion") && !has(&ev, "item.sold:potion"), "{ev:?}");
    assert_eq!(bag_count(&app, "potion"), 3);
    assert_eq!(global_gold(&app), 95.0);
    // Selling what fits works.
    let ev = run(&mut app, vec![sell("potion", 1, false)]);
    assert!(has(&ev, "item.sold:potion"), "{ev:?}");
    assert_eq!(global_gold(&app), 100.0);
}

#[test]
fn a_missing_currency_stat_refuses_the_sale() {
    let mut app = sell_app(0.0, 99_999.0, &[("potion", 2)]);
    app.world_mut().resource_mut::<LoadedStats>().0.shift_remove("gold");
    let ev = run(&mut app, vec![sell("potion", 1, false)]);
    assert!(has(&ev, "item.sell_refused:potion") && !has(&ev, "item.sold:potion"), "{ev:?}");
    assert_eq!(bag_count(&app, "potion"), 2);
}

#[test]
fn selling_does_not_restock_the_merchant() {
    // old_key: one in stock. Buy it, sell it back (this merchant pays 10), and it is still sold out: there is no buy-back.
    let mut app = sell_app(200.0, 99_999.0, &[]);
    let ev = run(&mut app, vec![Action::BuyItem("old_key".to_string())]);
    assert!(has(&ev, "item.bought:old_key"), "{ev:?}");
    let ev = run(&mut app, vec![sell("old_key", 1, false)]);
    assert!(has(&ev, "item.sold:old_key"), "this merchant's entry prices it: {ev:?}");
    assert_eq!(global_gold(&app), 160.0, "200 - 50 + 10");
    let ev = run(&mut app, vec![Action::BuyItem("old_key".to_string())]);
    assert!(has(&ev, "shop.sold_out:old_key") && !has(&ev, "item.bought:old_key"), "{ev:?}");
}

#[test]
fn destroying_removes_the_count_without_paying() {
    let mut app = sell_app(50.0, 99_999.0, &[("potion", 7)]);
    let ev = run(&mut app, vec![destroy("potion", 3, false)]);
    assert!(has(&ev, "item.destroyed:potion"), "{ev:?}");
    assert!(!has(&ev, "item.destroy_refused"), "{ev:?}");
    assert_eq!(bag_count(&app, "potion"), 4);
    assert_eq!(global_gold(&app), 50.0, "destroying pays nothing");
    let ev = run(&mut app, vec![destroy("potion", 1, true)]);
    assert!(has(&ev, "item.destroyed:potion"), "{ev:?}");
    assert_eq!(bag_count(&app, "potion"), 0, "`all` destroys every unit");
}

#[test]
fn destroying_works_with_no_shop_open() {
    let mut app = sell_app(0.0, 99_999.0, &[("pelt", 2)]);
    let _ = run(&mut app, vec![Action::CloseShop]);
    let ev = run(&mut app, vec![destroy("pelt", 2, false)]);
    assert!(has(&ev, "item.destroyed:pelt"), "{ev:?}");
    assert_eq!(bag_count(&app, "pelt"), 0);
}

#[test]
fn a_key_item_cannot_be_destroyed() {
    let mut app = sell_app(0.0, 99_999.0, &[("old_key", 1)]);
    let ev = run(&mut app, vec![destroy("old_key", 1, false)]);
    assert!(has(&ev, "item.destroy_refused") && has(&ev, "item.destroy_refused:old_key"), "{ev:?}");
    assert!(!has(&ev, "item.destroyed:old_key"), "{ev:?}");
    assert_eq!(bag_count(&app, "old_key"), 1);
}

#[test]
fn destroying_more_than_held_zero_or_an_unknown_item_is_refused() {
    let mut app = sell_app(0.0, 99_999.0, &[("potion", 2)]);
    for action in [destroy("potion", 3, false), destroy("potion", 0, false), destroy("nonsense", 1, false), destroy("pelt", 1, true)] {
        let ev = run(&mut app, vec![action.clone()]);
        assert!(has(&ev, "item.destroy_refused"), "{action:?}: {ev:?}");
        assert!(!ev.iter().any(|e| e.starts_with("item.destroyed:")), "{action:?}: {ev:?}");
    }
    assert_eq!(bag_count(&app, "potion"), 2, "all or nothing");
}

#[test]
fn the_bag_menu_events_run_the_built_in_sell_and_destroy_unless_a_rule_owns_them() {
    // sell_item = one unit, sell_stack = every unit, destroy_item = every unit (the menu's confirm happened before the event).
    let mut app = sell_app(0.0, 99_999.0, &[("potion", 4), ("pelt", 3)]);
    send_click(&mut app, "sell_item:potion");
    assert_eq!(bag_count(&app, "potion"), 3);
    assert_eq!(global_gold(&app), 5.0);
    send_click(&mut app, "sell_stack:potion");
    assert_eq!(bag_count(&app, "potion"), 0);
    assert_eq!(global_gold(&app), 20.0);
    send_click(&mut app, "destroy_item:pelt");
    assert_eq!(bag_count(&app, "pelt"), 0);

    // A rule bound on the event (in any state) owns it: the built-in does not run, so there is no double action.
    let mut app = sell_app(0.0, 99_999.0, &[("potion", 4), ("pelt", 3)]);
    for trigger in ["sell_item:potion", "destroy_item:pelt"] {
        app.world_mut().insert_resource(fsm_with("playing", Some(&format!("ui.button_pressed:{trigger}")), false));
        app.world_mut().resource_mut::<LogicState>().0 = "playing".to_string();
        app.update();
        send_click(&mut app, trigger);
        assert!(has(&events(&app), "rule.ran"), "{trigger}: {:?}", events(&app));
    }
    assert_eq!(bag_count(&app, "potion"), 4);
    assert_eq!(bag_count(&app, "pelt"), 3);
}

#[test]
fn the_sell_and_destroy_actions_parse_from_ron_with_defaults() {
    let a: Action = ron::from_str(r#"SellItem(item_key: "gem")"#).expect("count and all default");
    assert_eq!(a, Action::SellItem { item_key: "gem".to_string(), count: 1, all: false });
    let a: Action = ron::from_str(r#"DestroyItem(item_key: "gem", all: true)"#).expect("all");
    assert_eq!(a, Action::DestroyItem { item_key: "gem".to_string(), count: 1, all: true });
    let e: ShopEntry = ron::from_str(r#"(item_key: "gem", buy_price: 10)"#).expect("sell_price omitted");
    assert_eq!(e.sell_price, None);
    // The project's RON loader enables `implicit_some` (schema/ron_loader.rs), so an authored `sell_price: 5` is still a valid `Option<u32>`.
    let implicit = ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let e: ShopEntry = implicit.from_str(r#"(item_key: "gem", buy_price: 10, sell_price: 5)"#).expect("an old authored price still parses");
    assert_eq!(e.sell_price, Some(5));
    let d: ItemDef = ron::from_str(r#"(display_name: "Gem")"#).expect("defaults");
    assert!(d.destroyable && d.sell_price.is_none());
}

// ---- item system S4: the bag slot menu (`planning/features/done/item_sell_destroy.md`) --------------------------------------------------

/// A shop-open scene (`sell_app`) plus a visible InventoryPanel (3 x 2), the hidden menu overlay and six slot nodes.
fn menu_app(bag: &[(&str, u32)]) -> (App, Vec<Entity>) {
    let mut app = sell_app(0.0, 99_999.0, bag);
    app.world_mut().spawn((InventoryPanelMarker { columns: 3, rows: 2, font_size: 12.0 }, Visibility::Visible));
    app.world_mut().commands().spawn(Name::new("menu_host")).with_children(|p| spawn_slot_menu(p, 3, 2, 40.0, 12.0));
    app.world_mut().flush();
    let slots = (0..6).map(|i| slot(&mut app, i)).collect();
    app.update();
    (app, slots)
}

fn press(app: &mut App, e: Entity) {
    *app.world_mut().get_mut::<Interaction>(e).unwrap() = Interaction::Pressed;
    app.update();
    *app.world_mut().get_mut::<Interaction>(e).unwrap() = Interaction::None;
    app.update();
}

fn row_entity(app: &mut App, kind: MenuRowKind) -> Entity {
    let mut q = app.world_mut().query::<(Entity, &SlotMenuRow)>();
    q.iter(app.world()).find(|(_, r)| r.0 == kind).map(|(e, _)| e).expect("menu row exists")
}

fn row_shown(app: &mut App, kind: MenuRowKind) -> bool {
    let e = row_entity(app, kind);
    app.world().get::<Node>(e).unwrap().display == Display::Flex
}

fn menu_state(app: &App) -> Option<ironhold_core::capabilities::slot_menu::SlotMenu> {
    app.world().resource::<LoadedInventoryUi>().slot_menu.clone()
}

fn press_row(app: &mut App, kind: MenuRowKind) {
    let e = row_entity(app, kind);
    press(app, e);
}

/// Lets the real clock run past the Yes/No arming delay (a Yes pressed right after the question appeared is ignored).
fn wait_for_confirm(app: &mut App) {
    app.world_mut().resource_mut::<Time<Real>>().advance_by(std::time::Duration::from_millis(1000));
    app.update();
}

fn gold_now(app: &App) -> f32 {
    global_gold(app)
}

#[test]
fn a_left_click_opens_the_menu_and_uses_nothing() {
    let (mut app, slots) = menu_app(&[("potion", 3)]);
    app.world_mut().resource_mut::<Messages<GameEvent>>().clear();
    press(&mut app, slots[0]);
    let m = menu_state(&app).expect("the menu opens");
    assert_eq!((m.slot_index, m.item_key.as_str(), m.count), (0, "potion", 3));
    assert!(m.merchant.is_some(), "a shop is open");
    assert_eq!(bag_count(&app, "potion"), 3, "opening the menu changes nothing");
    assert!(events(&app).iter().all(|e| !e.starts_with("item.")), "{:?}", events(&app));
    // A potion in this catalog has no `use:` and a price: Sell 1, Sell 3 and Destroy are offered, Use is not.
    assert!(row_shown(&mut app, MenuRowKind::SellOne) && row_shown(&mut app, MenuRowKind::SellStack) && row_shown(&mut app, MenuRowKind::Destroy));
    assert!(!row_shown(&mut app, MenuRowKind::Use) && !row_shown(&mut app, MenuRowKind::Yes));
    // Clicking the open slot again closes it.
    press(&mut app, slots[0]);
    assert!(menu_state(&app).is_none());
}

#[test]
fn the_menu_rows_follow_the_shop_the_use_block_and_the_stack() {
    let (mut app, slots) = menu_app(&[("sword", 1)]);
    // No shop: no sell rows.
    let _ = run(&mut app, vec![Action::CloseShop]);
    press(&mut app, slots[0]);
    assert!(menu_state(&app).is_some());
    assert!(!row_shown(&mut app, MenuRowKind::SellOne) && !row_shown(&mut app, MenuRowKind::SellStack), "no shop open");
    assert!(row_shown(&mut app, MenuRowKind::Destroy));
    // A single unit: Sell, but no Sell N.
    let _ = run(&mut app, vec![Action::OpenShop("merchant_01".to_string())]);
    assert!(menu_state(&app).is_none(), "the shop opening closed the menu");
    press(&mut app, slots[0]);
    assert!(row_shown(&mut app, MenuRowKind::SellOne) && !row_shown(&mut app, MenuRowKind::SellStack));
}

#[test]
fn a_key_item_menu_has_no_verbs() {
    // old_key is destroyable: false; the merchant of `sell_app` prices it, so use a bag where no shop is open.
    let (mut app, slots) = menu_app(&[("old_key", 1)]);
    let _ = run(&mut app, vec![Action::CloseShop]);
    press(&mut app, slots[0]);
    assert!(menu_state(&app).is_some());
    assert!(row_shown(&mut app, MenuRowKind::NoVerbs), "a dimmed 'Cannot be sold or destroyed' line");
    for kind in [MenuRowKind::Use, MenuRowKind::SellOne, MenuRowKind::SellStack, MenuRowKind::Destroy] {
        assert!(!row_shown(&mut app, kind), "{kind:?}");
    }
}

#[test]
fn the_sell_row_sells_one_unit_and_closes_the_menu() {
    let (mut app, slots) = menu_app(&[("potion", 3)]);
    press(&mut app, slots[0]);
    press_row(&mut app, MenuRowKind::SellOne);
    assert_eq!(bag_count(&app, "potion"), 2);
    assert_eq!(gold_now(&app), 5.0);
    assert!(menu_state(&app).is_none(), "a verb closes the menu");
}

#[test]
fn sell_n_asks_first_and_sells_only_the_clicked_stack() {
    // 25 potions at 10 per stack = stacks of 10, 10 and 5. Clicking the second stack sells 10, not 25.
    let (mut app, slots) = menu_app(&[("potion", 25)]);
    press(&mut app, slots[1]);
    assert_eq!(menu_state(&app).unwrap().count, 10);
    press_row(&mut app, MenuRowKind::SellStack);
    assert_eq!(bag_count(&app, "potion"), 25, "the question comes first, nothing is sold yet");
    assert_eq!(menu_state(&app).unwrap().confirm, Some(MenuConfirm::SellStack));
    assert!(row_shown(&mut app, MenuRowKind::Prompt) && row_shown(&mut app, MenuRowKind::Yes) && !row_shown(&mut app, MenuRowKind::SellOne));
    wait_for_confirm(&mut app);
    press_row(&mut app, MenuRowKind::Yes);
    assert_eq!(bag_count(&app, "potion"), 15, "only the clicked stack of 10");
    assert_eq!(gold_now(&app), 50.0);
    assert!(menu_state(&app).is_none());
    assert!(app.world().resource::<LoadedInventoryUi>().menu_action.is_none(), "the quantity was consumed");
}

#[test]
fn destroy_needs_a_yes_and_no_backs_out() {
    let (mut app, slots) = menu_app(&[("pelt", 4)]);
    let _ = run(&mut app, vec![Action::CloseShop]);
    press(&mut app, slots[0]);
    press_row(&mut app, MenuRowKind::Destroy);
    assert_eq!(bag_count(&app, "pelt"), 4, "Destroy only asks");
    press_row(&mut app, MenuRowKind::No);
    assert_eq!(bag_count(&app, "pelt"), 4);
    let m = menu_state(&app).expect("No goes back to the verbs, the menu stays");
    assert_eq!(m.confirm, None);
    assert!(row_shown(&mut app, MenuRowKind::Destroy) && !row_shown(&mut app, MenuRowKind::Yes));
    press_row(&mut app, MenuRowKind::Destroy);
    wait_for_confirm(&mut app);
    press_row(&mut app, MenuRowKind::Yes);
    assert_eq!(bag_count(&app, "pelt"), 0, "Yes destroys the clicked stack");
    assert!(menu_state(&app).is_none());
}

#[test]
fn a_press_outside_the_menu_closes_it() {
    let (mut app, slots) = menu_app(&[("potion", 3)]);
    press(&mut app, slots[0]);
    assert!(menu_state(&app).is_some());
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    app.update();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
    assert!(menu_state(&app).is_none());
}

#[test]
fn the_menu_closes_when_its_state_goes_stale() {
    use ironhold_core::runtime::sim_clock::{PauseHold, SimClock};
    // the item is used up elsewhere
    let (mut app, slots) = menu_app(&[("potion", 3)]);
    press(&mut app, slots[0]);
    let _ = run(&mut app, vec![Action::RemoveItem { entity: "player".to_string(), item_key: "potion".to_string(), count: 1, all_or_nothing: false }]);
    assert!(menu_state(&app).is_none(), "the stack changed");
    // the shop closes (walking past the merchant)
    press(&mut app, slots[0]);
    assert!(menu_state(&app).is_some());
    let _ = run(&mut app, vec![Action::CloseShop]);
    assert!(menu_state(&app).is_none(), "a stale Sell row must not survive the shop closing");
    // the pause menu
    press(&mut app, slots[0]);
    assert!(menu_state(&app).is_some());
    app.world_mut().resource_mut::<SimClock>().hold(PauseHold::Menu);
    app.update();
    assert!(menu_state(&app).is_none());
    app.world_mut().resource_mut::<SimClock>().release(PauseHold::Menu);
    // the bag panel is hidden
    app.update();
    press(&mut app, slots[0]);
    assert!(menu_state(&app).is_some());
    let mut q = app.world_mut().query::<&mut Visibility>();
    for mut v in q.iter_mut(app.world_mut()) {
        *v = Visibility::Hidden;
    }
    app.update();
    assert!(menu_state(&app).is_none(), "a hidden panel closes the menu");
}

#[test]
fn right_click_still_uses_at_once_and_an_empty_slot_opens_nothing() {
    let (mut app, slots) = menu_app(&[("potion", 2)]);
    press(&mut app, slots[4]);
    assert!(menu_state(&app).is_none(), "an empty slot opens no menu");
    press(&mut app, slots[0]);
    assert!(menu_state(&app).is_some());
    *app.world_mut().get_mut::<Interaction>(slots[0]).unwrap() = Interaction::Hovered;
    app.update();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    app.update();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
    assert!(menu_state(&app).is_none(), "the shortcut closes the menu");
}

#[test]
fn a_rule_that_owns_destroy_item_gets_the_event_and_the_menu_quantity_does_not_leak() {
    let (mut app, slots) = menu_app(&[("pelt", 4)]);
    let _ = run(&mut app, vec![Action::CloseShop]);
    app.world_mut().insert_resource(fsm_with("playing", Some("ui.button_pressed:destroy_item:pelt"), false));
    app.world_mut().resource_mut::<LogicState>().0 = "playing".to_string();
    app.update();
    press(&mut app, slots[0]);
    press_row(&mut app, MenuRowKind::Destroy);
    app.world_mut().resource_mut::<Messages<GameEvent>>().clear();
    wait_for_confirm(&mut app);
    press_row(&mut app, MenuRowKind::Yes);
    assert_eq!(bag_count(&app, "pelt"), 4, "the rule owns the event: the built-in did not destroy");
    assert!(app.world().resource::<LoadedInventoryUi>().menu_action.is_none(), "the quantity was taken even though a rule owned the event");
}

#[test]
fn the_menu_opens_towards_the_middle_of_the_panel() {
    let (mut app, slots) = menu_app(&[("sword", 6)]);
    let root = app.world_mut().query::<(Entity, &ironhold_core::capabilities::slot_menu::SlotMenuRoot)>().iter(app.world()).next().map(|(e, _)| e).unwrap();
    // slot 0: top-left -> opens right and down
    press(&mut app, slots[0]);
    let n = app.world().get::<Node>(root).unwrap().clone();
    assert!(n.left != Val::Auto && n.right == Val::Auto && n.top != Val::Auto && n.bottom == Val::Auto, "{n:?}");
    assert_eq!(app.world().get::<ChildOf>(root).unwrap().parent(), slots[0], "the menu hangs under the clicked slot");
    // slot 5: bottom-right -> opens left and up
    press(&mut app, slots[0]);
    press(&mut app, slots[5]);
    let n = app.world().get::<Node>(root).unwrap().clone();
    assert!(n.left == Val::Auto && n.right != Val::Auto && n.top == Val::Auto && n.bottom != Val::Auto, "{n:?}");
    assert_eq!(app.world().get::<ChildOf>(root).unwrap().parent(), slots[5]);
}

#[test]
fn a_yes_pressed_at_once_after_the_question_appears_is_ignored() {
    // The question replaces the verb rows in the same spot: the second click of a double-click on Destroy must not confirm it.
    let (mut app, slots) = menu_app(&[("pelt", 4)]);
    let _ = run(&mut app, vec![Action::CloseShop]);
    press(&mut app, slots[0]);
    press_row(&mut app, MenuRowKind::Destroy);
    press_row(&mut app, MenuRowKind::Yes);
    assert_eq!(bag_count(&app, "pelt"), 4, "too soon: nothing destroyed");
    assert_eq!(menu_state(&app).unwrap().confirm, Some(MenuConfirm::Destroy), "the question stays");
    wait_for_confirm(&mut app);
    press_row(&mut app, MenuRowKind::Yes);
    assert_eq!(bag_count(&app, "pelt"), 0);
}

#[test]
fn an_authored_button_cannot_take_the_menus_quantity_for_another_verb() {
    // The menu confirms "Sell 5": quantity 5 belongs to `sell_stack:` only. An authored `destroy_item:x` press the same frame
    // is a different verb and acts on every unit, and the sell keeps its own quantity.
    use ironhold_core::capabilities::panel_defaults::{DESTROY_ITEM_PREFIX, SELL_STACK_PREFIX};
    let (mut app, _slots) = menu_app(&[("potion", 8)]);
    app.world_mut().resource_mut::<LoadedInventoryUi>().menu_action = Some((SELL_STACK_PREFIX, "potion".to_string(), 5));
    send_click(&mut app, &format!("{DESTROY_ITEM_PREFIX}potion"));
    assert_eq!(bag_count(&app, "potion"), 0, "the destroy had no menu quantity: every unit");
    assert!(app.world().resource::<LoadedInventoryUi>().menu_action.is_some(), "the sell's quantity is still waiting for its own event");
}

#[test]
fn an_item_the_catalog_does_not_know_gets_no_destroy_row() {
    // With no items.ron the executor refuses DestroyItem for every key, so the menu must not offer it.
    let (mut app, slots) = menu_app(&[("potion", 2)]);
    let _ = run(&mut app, vec![Action::CloseShop]);
    app.world_mut().insert_resource(LoadedItemCatalog(None));
    press(&mut app, slots[0]);
    assert!(menu_state(&app).is_some());
    assert!(!row_shown(&mut app, MenuRowKind::Destroy));
    assert!(row_shown(&mut app, MenuRowKind::NoVerbs));
}

#[test]
fn a_sell_n_question_closes_when_the_shop_stops_paying() {
    // The merchant despawning mid-question must not leave "Sell 3 for 0 gold?" on screen.
    let (mut app, slots) = menu_app(&[("potion", 3)]);
    press(&mut app, slots[0]);
    press_row(&mut app, MenuRowKind::SellStack);
    assert_eq!(menu_state(&app).unwrap().confirm, Some(MenuConfirm::SellStack));
    let merchant = app.world().resource::<SpawnRegistry>().entities["merchant_01"];
    app.world_mut().despawn(merchant);
    app.update();
    assert!(menu_state(&app).is_none());
}

#[test]
fn selling_one_of_several_non_stackable_swords_sells_exactly_one() {
    // Three swords over three slots (a non-stackable item is one per slot). Clicking one sword and choosing Sell must sell one.
    let (mut app, slots) = menu_app(&[("sword", 3)]);
    assert_eq!(app.world().resource::<PlayerInventory>().slots.iter().flatten().count(), 3, "one sword per slot");
    press(&mut app, slots[1]);
    let m = menu_state(&app).expect("menu");
    assert_eq!((m.count, m.total), (1, 3));
    assert!(row_shown(&mut app, MenuRowKind::SellOne));
    press_row(&mut app, MenuRowKind::SellOne);
    assert_eq!(bag_count(&app, "sword"), 2, "only one sword sold");
    assert_eq!(gold_now(&app), 30.0);
}

#[test]
fn several_swords_offer_sell_all_and_it_asks_first() {
    // Three non-stackable swords over three slots: Sell sells one, Sell all (3) sells them all after a Yes.
    let (mut app, slots) = menu_app(&[("sword", 3)]);
    press(&mut app, slots[0]);
    assert!(row_shown(&mut app, MenuRowKind::SellOne) && row_shown(&mut app, MenuRowKind::SellAll));
    assert!(!row_shown(&mut app, MenuRowKind::SellStack), "the clicked stack is a single sword: no Sell N");
    press_row(&mut app, MenuRowKind::SellAll);
    assert_eq!(bag_count(&app, "sword"), 3, "the question comes first");
    assert_eq!(menu_state(&app).unwrap().confirm, Some(MenuConfirm::SellAll));
    wait_for_confirm(&mut app);
    press_row(&mut app, MenuRowKind::Yes);
    assert_eq!(bag_count(&app, "sword"), 0, "every sword sold");
    assert_eq!(gold_now(&app), 90.0, "3 x 30");
    assert!(menu_state(&app).is_none());
}

#[test]
fn destroy_all_removes_every_unit_over_every_stack() {
    let (mut app, slots) = menu_app(&[("potion", 25)]);
    let _ = run(&mut app, vec![Action::CloseShop]);
    press(&mut app, slots[0]);
    assert!(row_shown(&mut app, MenuRowKind::Destroy) && row_shown(&mut app, MenuRowKind::DestroyAll));
    press_row(&mut app, MenuRowKind::DestroyAll);
    assert_eq!(bag_count(&app, "potion"), 25);
    wait_for_confirm(&mut app);
    press_row(&mut app, MenuRowKind::Yes);
    assert_eq!(bag_count(&app, "potion"), 0, "all 25, over three stacks");
}

#[test]
fn a_single_stack_offers_no_all_rows() {
    let (mut app, slots) = menu_app(&[("potion", 3)]);
    press(&mut app, slots[0]);
    assert!(!row_shown(&mut app, MenuRowKind::SellAll) && !row_shown(&mut app, MenuRowKind::DestroyAll));
}

#[test]
fn the_all_events_run_the_built_in_unless_a_rule_owns_them() {
    let (mut app, _slots) = menu_app(&[("sword", 3), ("pelt", 2)]);
    send_click(&mut app, "sell_all:sword");
    assert_eq!(bag_count(&app, "sword"), 0);
    assert_eq!(gold_now(&app), 90.0);
    send_click(&mut app, "destroy_all:pelt");
    assert_eq!(bag_count(&app, "pelt"), 0);

    let (mut app, _slots) = menu_app(&[("sword", 3)]);
    app.world_mut().insert_resource(fsm_with("playing", Some("ui.button_pressed:sell_all:sword"), false));
    app.world_mut().resource_mut::<LogicState>().0 = "playing".to_string();
    app.update();
    send_click(&mut app, "sell_all:sword");
    assert!(has(&events(&app), "rule.ran"));
    assert_eq!(bag_count(&app, "sword"), 3, "the rule owns the event: the built-in did not sell");
}

// ---- item system S5: item slots on the action bar (`planning/features/done/action_bar_item_slots.md`) ---------------------------------------

use ironhold_core::capabilities::action_bar::{
    ActionSlotItem, ActionSlotUi, CooldownMap, CooldownOverlay, SlotIcon, SlotItemCount,
};

fn bar_player() -> CharacterController {
    CharacterController {
        walk_speed: 5.0, run_speed: 8.0, rot_speed: 2.0,
        inputs: InputMap { gamepad_deadzone: 0.15, ..InputMap::default() },
        is_running: false, jump_velocity: 5.94, double_jump_enabled: false,
        double_jump_velocity: 5.94, jumps_used: 0, max_jumps: 1,
        collider_radius: 0.4, ground_cast_length: 0.3, max_walkable_slope_deg: 45.0, coyote_time_secs: 0.1,
        coyote_ticks_remaining: 0, idle_drag: 0.8, jump_air_grace: 0, jump_liftoff_y: None,
    }
}

struct BarItem {
    app: App,
    slot: Entity,
    count: Entity,
    icon: Entity,
    overlay: Entity,
}

/// A scene with a player, the use catalog, a bag holding `bag` and one item slot "6" for a potion (cooldown 5 s), with its count,
/// icon and cooldown-overlay children as the loader builds them.
fn bar_item_app(bag: &[(&str, u32)]) -> BarItem {
    let mut app = use_app(bag);
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        bar_player(),
        ironhold_core::capabilities::player::PlayerTarget::default(),
    ));
    let slot = app.world_mut().spawn((
        ActionSlotUi {
            slot_key: "6".to_string(),
            resolved_key: Some(KeyCode::Digit6),
            resolved_gamepad_button: None,
            resolved_mouse_button: None,
            do_actions: vec![], // the item component is the one source of what the slot does
            cooldown_secs: Some(5.0),
            cost: None,
            owner_player: None,
        },
        ActionSlotItem("potion".to_string()),
        Interaction::None,
    )).id();
    let count = app.world_mut().spawn((Text::new(""), TextColor(Color::WHITE), SlotItemCount::default(), ChildOf(slot))).id();
    let icon = app.world_mut().spawn((
        ImageNode { texture_atlas: Some(TextureAtlas { layout: Handle::default(), index: 0 }), ..default() },
        Visibility::Hidden,
        SlotIcon { tint_override: None },
        ChildOf(slot),
    )).id();
    let overlay = app.world_mut().spawn((
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        CooldownOverlay { slot_key: "6".to_string() },
        ChildOf(slot),
    )).id();
    app.update();
    BarItem { app, slot, count, icon, overlay }
}

/// Presses the slot once (the `Interaction` edge) and returns the events that frame emitted.
fn press_bar_slot(b: &mut BarItem) -> Vec<String> {
    b.app.world_mut().resource_mut::<Messages<GameEvent>>().clear();
    *b.app.world_mut().get_mut::<Interaction>(b.slot).unwrap() = Interaction::Pressed;
    b.app.update();
    let ev = events(&b.app);
    *b.app.world_mut().get_mut::<Interaction>(b.slot).unwrap() = Interaction::None;
    b.app.update();
    ev
}

fn count_text(b: &BarItem) -> String {
    b.app.world().get::<Text>(b.count).unwrap().0.clone()
}

fn overlay_alpha(b: &BarItem) -> f32 {
    b.app.world().get::<BackgroundColor>(b.overlay).unwrap().0.alpha()
}

#[test]
fn pressing_an_item_slot_uses_the_item_through_the_intent_path() {
    let mut b = bar_item_app(&[("potion", 3)]);
    let ev = press_bar_slot(&mut b);
    assert!(has(&ev, "action_bar.pressed:6"), "{ev:?}");
    assert!(has(&ev, "intent.slot.6:player_01"), "the usual intent event: {ev:?}");
    assert!(has(&ev, "item.used:potion"), "{ev:?}");
    assert_eq!(bag_count(&b.app, "potion"), 2);
    assert!(b.app.world().resource::<CooldownMap>().0.contains_key("6"), "the slot's cooldown started");
}

#[test]
fn an_empty_bag_refuses_with_no_item_and_starts_nothing() {
    let mut b = bar_item_app(&[("sword", 1)]);
    let ev = press_bar_slot(&mut b);
    assert!(has(&ev, "action_bar.no_item:6"), "{ev:?}");
    for not in ["action_bar.pressed:6", "intent.slot.6:player_01", "action_bar.activated:6", "item.used:potion", "item.use_failed"] {
        assert!(!has(&ev, not), "{not} must not fire on a refusal: {ev:?}");
    }
    assert!(!b.app.world().resource::<CooldownMap>().0.contains_key("6"), "a refusal must not start the cooldown");
}

#[test]
fn the_no_item_gate_sits_between_the_cooldown_and_the_cost() {
    // cooldown first: an empty slot that is cooling down says on_cooldown
    let mut b = bar_item_app(&[]);
    b.app.world_mut().resource_mut::<CooldownMap>().0.insert("6".to_string(), (3.0, 5.0));
    let ev = press_bar_slot(&mut b);
    assert!(has(&ev, "action_bar.on_cooldown:6") && !has(&ev, "action_bar.no_item:6"), "{ev:?}");
    // then no-item, then cost: an empty slot with an unaffordable cost says no_item
    let mut b = bar_item_app(&[]);
    b.app.world_mut().get_mut::<ActionSlotUi>(b.slot).unwrap().cost =
        Some(ironhold_core::schema::scene_v2::SlotCost { stat: "mana".to_string(), amount: 20.0 });
    let ev = press_bar_slot(&mut b);
    assert!(has(&ev, "action_bar.no_item:6") && !has(&ev, "action_bar.insufficient_resource:6"), "{ev:?}");
    // with the item held the cost gate speaks
    let mut b = bar_item_app(&[("potion", 1)]);
    b.app.world_mut().get_mut::<ActionSlotUi>(b.slot).unwrap().cost =
        Some(ironhold_core::schema::scene_v2::SlotCost { stat: "mana".to_string(), amount: 20.0 });
    let ev = press_bar_slot(&mut b);
    assert!(has(&ev, "action_bar.insufficient_resource:6") && !has(&ev, "item.used:potion"), "{ev:?}");
    assert_eq!(bag_count(&b.app, "potion"), 1, "a blocked slot spends nothing");
}

#[test]
fn a_rule_on_the_intent_replaces_an_item_slot_like_any_slot() {
    let mut b = bar_item_app(&[("potion", 3)]);
    b.app.world_mut().insert_resource(fsm_with("playing", Some("intent.slot.6:player_01"), false));
    b.app.world_mut().resource_mut::<LogicState>().0 = "playing".to_string();
    b.app.update();
    let ev = press_bar_slot(&mut b);
    assert!(has(&ev, "rule.ran"), "{ev:?}");
    assert!(!has(&ev, "item.used:potion"), "the rule replaced the built-in use: {ev:?}");
    assert_eq!(bag_count(&b.app, "potion"), 3);
}

#[test]
fn the_count_label_shows_the_total_over_every_stack_and_follows_the_bag_at_once() {
    // 25 potions at 99 per stack is one stack in this catalog-less bag; use two items' worth: 3 + 12
    let mut b = bar_item_app(&[("potion", 15)]);
    assert_eq!(count_text(&b), "15");
    let _ = press_bar_slot(&mut b);
    assert_eq!(count_text(&b), "14", "the label follows a use the same frame");
    let _ = run(&mut b.app, vec![Action::AddItem { entity: "player".to_string(), item_key: "potion".to_string(), count: 200 }]);
    assert_eq!(count_text(&b), "99+", "capped so it never outgrows the slot");
    let color = b.app.world().get::<TextColor>(b.count).unwrap().0;
    assert_ne!(color, Color::srgba(0.55, 0.55, 0.55, 0.9), "not grey while held");
}

#[test]
fn an_empty_item_slot_is_dimmed_with_a_grey_zero_and_a_cooldown_still_wins() {
    let mut b = bar_item_app(&[("potion", 1)]);
    b.app.update(); // the first frame draws before the count is first read
    assert!(overlay_alpha(&b) < 0.01, "one held: not dimmed");
    let _ = press_bar_slot(&mut b); // uses the last potion, starts the cooldown
    assert_eq!(count_text(&b), "0");
    assert_eq!(b.app.world().get::<TextColor>(b.count).unwrap().0, Color::srgba(0.55, 0.55, 0.55, 0.9), "a grey 0");
    // the cooldown (alpha 0.25 + 0.45 * fraction) wins while it runs
    assert!(overlay_alpha(&b) > 0.6, "cooldown overlay while cooling down: {}", overlay_alpha(&b));
    b.app.world_mut().resource_mut::<CooldownMap>().0.clear();
    b.app.update();
    assert!((overlay_alpha(&b) - 0.45).abs() < 0.01, "empty and ready: the 0.45 dim, got {}", overlay_alpha(&b));
    // buying one more un-dims it
    let _ = run(&mut b.app, vec![Action::AddItem { entity: "player".to_string(), item_key: "potion".to_string(), count: 1 }]);
    b.app.update();
    assert!(overlay_alpha(&b) < 0.01);
}

#[test]
fn the_icon_comes_from_the_item_and_the_inventory_sheet_and_a_slot_tint_wins() {
    let mut b = bar_item_app(&[("potion", 2)]);
    // no InventoryPanel has loaded a sheet: nothing to show
    assert_eq!(*b.app.world().get::<Visibility>(b.icon).unwrap(), Visibility::Hidden);
    // the panel's sheet and the item's icon index and colour
    let mut catalog = use_catalog();
    {
        let d = catalog.items.get_mut("potion").unwrap();
        d.icon_index = 7;
        d.icon_color = Some((1.0, 0.0, 0.0, 1.0));
    }
    b.app.world_mut().insert_resource(LoadedItemCatalog(Some(catalog)));
    {
        let mut ui = b.app.world_mut().resource_mut::<LoadedInventoryUi>();
        ui.panel_icon_sheet = Some("icons_items".to_string());
        ui.icon_atlases.insert("icons_items".to_string(), (Handle::<Image>::default(), Handle::<TextureAtlasLayout>::default()));
    }
    b.app.update();
    let img = b.app.world().get::<ImageNode>(b.icon).unwrap();
    assert_eq!(img.texture_atlas.as_ref().unwrap().index, 7, "the item's own icon index");
    assert_eq!(img.color, Color::srgba(1.0, 0.0, 0.0, 1.0), "the item's tint");
    assert_eq!(*b.app.world().get::<Visibility>(b.icon).unwrap(), Visibility::Inherited);
    // a slot's own icon_color overrides the item's tint
    b.app.world_mut().get_mut::<SlotIcon>(b.icon).unwrap().tint_override = Some(Color::srgba(0.0, 0.0, 1.0, 1.0));
    b.app.world_mut().resource_mut::<LoadedItemCatalog>().set_changed();
    b.app.update();
    assert_eq!(b.app.world().get::<ImageNode>(b.icon).unwrap().color, Color::srgba(0.0, 0.0, 1.0, 1.0));
}

#[test]
fn a_plain_slot_is_untouched_by_the_bag() {
    // A slot with no ActionSlotItem never emits no_item and fires its own do_actions even with an empty bag.
    let mut b = bar_item_app(&[]);
    b.app.world_mut().entity_mut(b.slot).remove::<ActionSlotItem>();
    b.app.world_mut().get_mut::<ActionSlotUi>(b.slot).unwrap().do_actions = vec![emit_action("plain.fired")];
    let ev = press_bar_slot(&mut b);
    assert!(has(&ev, "plain.fired") && !has(&ev, "action_bar.no_item:6"), "{ev:?}");
}

#[test]
fn an_item_slot_parses_from_ron_without_do_actions() {
    use ironhold_core::schema::scene_v2::ActionSlotDef;
    // The project's RON loader enables `implicit_some`, so `item: "x"` and `cooldown_secs: 2.0` are valid `Option`s.
    let opts = ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let d: ActionSlotDef = opts.from_str(r#"(key: "6", item: "health_potion", cooldown_secs: 2.0)"#).expect("an item slot needs no do_actions");
    assert_eq!(d.item.as_deref(), Some("health_potion"));
    assert!(d.do_actions.is_empty());
    let d: ActionSlotDef = opts.from_str(r#"(key: "1", do_actions: [])"#).expect("an old slot still parses");
    assert!(d.item.is_none());
    let d: ActionSlotDef = opts.from_str(r#"(key: "6")"#).expect("an empty slot (the shape S6 wants)");
    assert!(d.do_actions.is_empty() && d.item.is_none());
}

#[test]
fn an_item_that_needs_more_than_is_held_is_refused_before_the_cost_and_the_cooldown() {
    // `elixir` consumes 2: with one held, the press must not spend the cost or start the cooldown only for `UseItem` to fail.
    let mut b = bar_item_app(&[("elixir", 1)]);
    b.app.world_mut().entity_mut(b.slot).insert(ActionSlotItem("elixir".to_string()));
    b.app.world_mut().get_mut::<ActionSlotUi>(b.slot).unwrap().cost =
        Some(ironhold_core::schema::scene_v2::SlotCost { stat: "gold".to_string(), amount: 5.0 });
    b.app.world_mut().resource_mut::<LoadedStats>().0.insert("gold".to_string(), gold(50.0, 99_999.0));
    b.app.update();
    let ev = press_bar_slot(&mut b);
    assert!(has(&ev, "action_bar.no_item:6"), "fewer held than one use takes: {ev:?}");
    assert!(!has(&ev, "item.use_failed") && !has(&ev, "action_bar.pressed:6"), "{ev:?}");
    assert!(!b.app.world().resource::<CooldownMap>().0.contains_key("6"));
    assert_eq!(global_gold(&b.app), 50.0, "no cost spent");
    assert!(overlay_alpha(&b) > 0.4, "dimmed: one held is not enough");
    // with two held it works and the cost is spent
    let _ = run(&mut b.app, vec![Action::AddItem { entity: "player".to_string(), item_key: "elixir".to_string(), count: 1 }]);
    let ev = press_bar_slot(&mut b);
    assert!(has(&ev, "item.used:elixir"), "{ev:?}");
    assert_eq!(global_gold(&b.app), 45.0);
}

#[test]
fn removing_the_item_clears_the_label_and_the_icon_and_the_dim() {
    let mut b = bar_item_app(&[]);
    let mut catalog = use_catalog();
    catalog.items.get_mut("potion").unwrap().icon_index = 7;
    b.app.world_mut().insert_resource(LoadedItemCatalog(Some(catalog)));
    {
        let mut ui = b.app.world_mut().resource_mut::<LoadedInventoryUi>();
        ui.panel_icon_sheet = Some("icons_items".to_string());
        ui.icon_atlases.insert("icons_items".to_string(), (Handle::<Image>::default(), Handle::<TextureAtlasLayout>::default()));
    }
    b.app.update();
    b.app.update();
    assert_eq!(count_text(&b), "0");
    assert!(overlay_alpha(&b) > 0.4, "an empty item slot is dimmed");
    assert_eq!(*b.app.world().get::<Visibility>(b.icon).unwrap(), Visibility::Inherited);
    // S6 clears the slot: no `ActionSlotItem` any more
    b.app.world_mut().entity_mut(b.slot).remove::<ActionSlotItem>();
    b.app.update();
    b.app.update();
    assert_eq!(count_text(&b), "", "the label is cleared");
    assert_eq!(*b.app.world().get::<Visibility>(b.icon).unwrap(), Visibility::Hidden, "the icon is hidden");
    assert!(overlay_alpha(&b) < 0.01, "no dim on a slot with no item");
    let ev = press_bar_slot(&mut b);
    assert!(!has(&ev, "action_bar.no_item:6") && !has(&ev, "item.used:potion"), "a slot with no item and no do_actions does nothing: {ev:?}");
}

#[test]
fn with_no_item_catalog_the_slot_reads_zero_has_no_icon_and_refuses() {
    let mut b = bar_item_app(&[("potion", 3)]);
    b.app.world_mut().insert_resource(LoadedItemCatalog(None));
    b.app.update();
    b.app.update();
    // the bag still holds three, the slot counts what the bag holds
    assert_eq!(count_text(&b), "3");
    assert_eq!(*b.app.world().get::<Visibility>(b.icon).unwrap(), Visibility::Hidden, "no catalog, no icon");
    // an item the catalog does not know
    b.app.world_mut().entity_mut(b.slot).insert(ActionSlotItem("nonsense".to_string()));
    b.app.update();
    b.app.update();
    assert_eq!(count_text(&b), "0");
    let ev = press_bar_slot(&mut b);
    assert!(has(&ev, "action_bar.no_item:6"), "{ev:?}");
}

// ---- item system S6: assignable action bar slots (`planning/features/done/action_bar_assignable_slots.md`) ------------------------------------

use ironhold_core::capabilities::bar_assign::{
    spawn_assign_strip, ActionBarAssignments, AssignMode, AssignStripButton, AssignStripRoot, AssignableSlot, Assignment,
};

struct AssignApp {
    app: App,
    bag: Vec<Entity>,
    s7: Entity, // assignable, starts empty
    s8: Entity, // assignable, starts empty
    s1: Entity, // a plain slot with do_actions
    panel: Entity,
}

fn spawn_bar_slot(app: &mut App, key: &str, code: KeyCode, assignable: bool, do_actions: Vec<Action>) -> Entity {
    let mut e = app.world_mut().spawn((
        ActionSlotUi {
            slot_key: key.to_string(),
            resolved_key: Some(code),
            resolved_gamepad_button: None,
            resolved_mouse_button: None,
            do_actions,
            cooldown_secs: Some(5.0),
            cost: None,
            owner_player: None,
        },
        Interaction::None,
    ));
    if assignable {
        e.insert(AssignableSlot { bar: "main".to_string(), key: key.to_string(), default: None });
    }
    let slot = e.id();
    app.world_mut().spawn((Text::new(""), TextColor(Color::WHITE), SlotItemCount::default(), ChildOf(slot)));
    app.world_mut().spawn((
        ImageNode { texture_atlas: Some(TextureAtlas { layout: Handle::default(), index: 0 }), ..default() },
        Visibility::Hidden,
        SlotIcon { tint_override: None },
        ChildOf(slot),
    ));
    app.world_mut().spawn((
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        CooldownOverlay { slot_key: key.to_string() },
        ChildOf(slot),
    ));
    slot
}

/// A scene with a player, the use catalog, a bag of `bag`, a visible bag panel (with the bag menu and the assign strip), six bag slot nodes,
/// two empty assignable bar slots "7" and "8" on bar "main", and a plain bar slot "1" that emits `plain.fired`.
fn assign_app(bag: &[(&str, u32)]) -> AssignApp {
    let mut app = use_app(bag);
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        bar_player(),
        ironhold_core::capabilities::player::PlayerTarget::default(),
    ));
    let panel = app.world_mut().spawn((InventoryPanelMarker { columns: 3, rows: 2, font_size: 12.0 }, Visibility::Visible)).id();
    app.world_mut().commands().spawn(Name::new("menu_host")).with_children(|p| {
        spawn_slot_menu(p, 3, 2, 40.0, 12.0);
        spawn_assign_strip(p, 12.0);
    });
    app.world_mut().flush();
    let bag_nodes = (0..6).map(|i| slot(&mut app, i)).collect();
    let s7 = spawn_bar_slot(&mut app, "7", KeyCode::Digit7, true, vec![]);
    let s8 = spawn_bar_slot(&mut app, "8", KeyCode::Digit8, true, vec![]);
    let s1 = spawn_bar_slot(&mut app, "1", KeyCode::Digit1, false, vec![emit_action("plain.fired")]);
    app.update();
    app.update();
    AssignApp { app, bag: bag_nodes, s7, s8, s1, panel }
}

fn settle(a: &mut AssignApp) {
    a.app.update();
    a.app.update();
}

fn mode(a: &AssignApp) -> Option<AssignMode> {
    a.app.world().resource::<LoadedInventoryUi>().assign_mode.clone()
}

fn slot_item(a: &AssignApp, slot: Entity) -> Option<String> {
    a.app.world().get::<ActionSlotItem>(slot).map(|i| i.0.clone())
}

fn assignment(a: &AssignApp, key: &str) -> Option<Assignment> {
    a.app.world().resource::<ActionBarAssignments>().get("main", key).cloned()
}

/// Presses an entity once (the `Interaction` edge), returns the events of that frame, then lets the world settle.
fn press_entity(a: &mut AssignApp, e: Entity) -> Vec<String> {
    a.app.world_mut().resource_mut::<Messages<GameEvent>>().clear();
    *a.app.world_mut().get_mut::<Interaction>(e).unwrap() = Interaction::Pressed;
    a.app.update();
    let ev = events(&a.app);
    *a.app.world_mut().get_mut::<Interaction>(e).unwrap() = Interaction::None;
    a.app.update();
    ev
}

fn strip_button(a: &mut AssignApp, kind: AssignStripButton) -> Entity {
    let mut q = a.app.world_mut().query::<(Entity, &AssignStripButton)>();
    q.iter(a.app.world()).find(|(_, b)| **b == kind).map(|(e, _)| e).expect("strip button")
}

fn strip_shown(a: &mut AssignApp) -> bool {
    let mut q = a.app.world_mut().query_filtered::<&Node, With<AssignStripRoot>>();
    q.iter(a.app.world()).next().is_some_and(|n| n.display == Display::Flex)
}

/// Opens the bag menu on the first bag slot and chooses Put on bar.
fn put_on_bar(a: &mut AssignApp) {
    let first = a.bag[0];
    let _ = press_entity(a, first);
    let row = row_entity(&mut a.app, MenuRowKind::PutOnBar);
    let _ = press_entity(a, row);
}

#[test]
fn put_on_bar_starts_the_mode_and_a_click_on_an_assignable_slot_assigns_without_using_it() {
    let mut a = assign_app(&[("potion", 3)]);
    let first = a.bag[0];
    let _ = press_entity(&mut a, first);
    assert!(row_shown(&mut a.app, MenuRowKind::PutOnBar), "a usable item and an assignable slot in the scene: the row is offered");
    let row = row_entity(&mut a.app, MenuRowKind::PutOnBar);
    let _ = press_entity(&mut a, row);
    assert_eq!(mode(&a), Some(AssignMode { item: "potion".to_string(), from: None }));
    assert!(strip_shown(&mut a), "the strip is shown in the mode");
    assert!(menu_state(&a.app).is_none(), "the menu closed");

    let s7 = a.s7;
    let ev = press_entity(&mut a, s7);
    assert!(has(&ev, "action_bar.slot_assigned:7") && has(&ev, "action_bar.slot_assigned:7:potion"), "{ev:?}");
    assert!(!has(&ev, "action_bar.empty_slot:7") && !has(&ev, "item.used:potion"), "the click assigned, it did not use the slot: {ev:?}");
    settle(&mut a);
    assert_eq!(slot_item(&a, a.s7), Some("potion".to_string()));
    assert_eq!(assignment(&a, "7"), Some(Assignment::Assigned("potion".to_string())));
    assert_eq!(mode(&a), None, "the mode ended (in Execute)");
    assert!(!strip_shown(&mut a));
    assert_eq!(bag_count(&a.app, "potion"), 3, "nothing was used");
}

#[test]
fn an_item_with_no_use_block_or_no_assignable_slot_gets_no_put_on_bar_row() {
    let mut a = assign_app(&[("sword", 1)]);
    let first = a.bag[0];
    let _ = press_entity(&mut a, first);
    assert!(!row_shown(&mut a.app, MenuRowKind::PutOnBar), "a sword has no use: block");
}

#[test]
fn the_assign_actions_write_the_resource_and_the_slot_follows() {
    let mut a = assign_app(&[]);
    let ev = run(&mut a.app, vec![Action::AssignBarSlot { bar: "main".to_string(), slot: "8".to_string(), item_key: "potion".to_string() }]);
    assert!(has(&ev, "action_bar.slot_assigned:8") && has(&ev, "action_bar.slot_assigned:8:potion"), "{ev:?}");
    settle(&mut a);
    assert_eq!(slot_item(&a, a.s8), Some("potion".to_string()), "the item need not be in the bag for the action");
    let ev = run(&mut a.app, vec![Action::ClearBarSlot { bar: "main".to_string(), slot: "8".to_string() }]);
    assert!(has(&ev, "action_bar.slot_cleared:8"), "{ev:?}");
    settle(&mut a);
    assert_eq!(slot_item(&a, a.s8), None);
    assert_eq!(assignment(&a, "8"), Some(Assignment::Cleared));
}

#[test]
fn the_assign_action_refuses_what_it_cannot_do_and_changes_nothing() {
    let mut a = assign_app(&[]);
    for (bar, slot, item) in [
        ("main", "9", "potion"),      // no such slot
        ("other", "7", "potion"),     // no such bar
        ("main", "1", "potion"),      // a plain slot is not assignable (it has no AssignableSlot)
        ("main", "7", "sword"),       // no use: block
        ("main", "7", "gold_coin"),   // a currency item
        ("main", "7", "nonsense"),    // not in the catalog
    ] {
        let ev = run(&mut a.app, vec![Action::AssignBarSlot { bar: bar.to_string(), slot: slot.to_string(), item_key: item.to_string() }]);
        assert!(has(&ev, &format!("action_bar.assign_failed:{slot}")), "{bar}/{slot}/{item}: {ev:?}");
        assert!(!ev.iter().any(|e| e.starts_with("action_bar.slot_assigned")), "{bar}/{slot}/{item}: {ev:?}");
    }
    assert!(a.app.world().resource::<ActionBarAssignments>().is_empty(), "nothing was written");
    let ev = run(&mut a.app, vec![Action::ClearBarSlot { bar: "main".to_string(), slot: "9".to_string() }]);
    assert!(has(&ev, "action_bar.assign_failed:9"), "{ev:?}");
}

#[test]
fn an_empty_assignable_slot_refuses_with_empty_slot_and_starts_nothing() {
    let mut a = assign_app(&[("potion", 1)]);
    let s7 = a.s7;
    let ev = press_entity(&mut a, s7);
    assert!(has(&ev, "action_bar.empty_slot:7"), "{ev:?}");
    for not in ["action_bar.pressed:7", "intent.slot.7:player_01", "action_bar.activated:7"] {
        assert!(!has(&ev, not), "{not}: {ev:?}");
    }
    assert!(!a.app.world().resource::<CooldownMap>().0.contains_key("7"), "an empty slot must not start a cooldown");
    // an empty assignable slot shows a faint +, a plain slot shows nothing
    let plus = {
        let mut q = a.app.world_mut().query::<(&ChildOf, &Text, &SlotItemCount)>();
        q.iter(a.app.world()).filter(|(p, t, _)| p.parent() == a.s7 && t.0 == "+").count()
    };
    assert_eq!(plus, 1, "a faint + marks the empty assignable slot");
}

#[test]
fn with_the_bag_open_a_filled_slot_is_picked_up_to_move_remove_or_put_back() {
    let mut a = assign_app(&[("potion", 3)]);
    let _ = run(&mut a.app, vec![Action::AssignBarSlot { bar: "main".to_string(), slot: "8".to_string(), item_key: "potion".to_string() }]);
    settle(&mut a);
    // click the filled slot with the bag open: it is picked up, not used
    let s8 = a.s8;
    let ev = press_entity(&mut a, s8);
    assert!(!has(&ev, "item.used:potion"), "{ev:?}");
    assert_eq!(mode(&a), Some(AssignMode { item: "potion".to_string(), from: Some(("main".to_string(), "8".to_string())) }));
    assert!(strip_shown(&mut a));
    // move it to slot 7
    let s7 = a.s7;
    let _ = press_entity(&mut a, s7);
    settle(&mut a);
    assert_eq!(slot_item(&a, a.s7), Some("potion".to_string()), "moved to 7");
    assert_eq!(slot_item(&a, a.s8), None, "and gone from 8");
    assert_eq!(mode(&a), None);
    // pick 7 up again and remove it from the bar
    let s7 = a.s7;
    let _ = press_entity(&mut a, s7);
    assert!(mode(&a).is_some());
    let remove = strip_button(&mut a, AssignStripButton::Remove);
    let _ = press_entity(&mut a, remove);
    settle(&mut a);
    assert_eq!(slot_item(&a, a.s7), None, "removed");
    assert_eq!(mode(&a), None);
    // pick up and put back on the same slot: nothing changes
    let _ = run(&mut a.app, vec![Action::AssignBarSlot { bar: "main".to_string(), slot: "8".to_string(), item_key: "potion".to_string() }]);
    settle(&mut a);
    let s8 = a.s8;
    let _ = press_entity(&mut a, s8);
    let _ = press_entity(&mut a, s8);
    settle(&mut a);
    assert_eq!(slot_item(&a, a.s8), Some("potion".to_string()));
    assert_eq!(mode(&a), None);
}

#[test]
fn a_slot_of_an_item_that_left_the_bag_can_still_be_picked_up_and_removed() {
    let mut a = assign_app(&[]);
    let _ = run(&mut a.app, vec![Action::AssignBarSlot { bar: "main".to_string(), slot: "8".to_string(), item_key: "potion".to_string() }]);
    settle(&mut a);
    let s8 = a.s8;
    let _ = press_entity(&mut a, s8);
    assert!(mode(&a).is_some(), "a grey-0 slot is picked up too");
    settle(&mut a);
    assert!(mode(&a).is_some(), "an item picked up from a slot does not cancel just because the bag has none");
    let remove = strip_button(&mut a, AssignStripButton::Remove);
    let _ = press_entity(&mut a, remove);
    settle(&mut a);
    assert_eq!(slot_item(&a, a.s8), None);
}

#[test]
fn with_the_bag_closed_a_click_uses_the_slot_and_a_key_always_does() {
    let mut a = assign_app(&[("potion", 5)]);
    let _ = run(&mut a.app, vec![Action::AssignBarSlot { bar: "main".to_string(), slot: "8".to_string(), item_key: "potion".to_string() }]);
    settle(&mut a);
    *a.app.world_mut().get_mut::<Visibility>(a.panel).unwrap() = Visibility::Hidden;
    settle(&mut a);
    let s8 = a.s8;
    let ev = press_entity(&mut a, s8);
    assert!(has(&ev, "item.used:potion"), "bag closed: a click uses it: {ev:?}");
    assert_eq!(mode(&a), None);
    // with the mode on, a key press still uses the slot
    *a.app.world_mut().get_mut::<Visibility>(a.panel).unwrap() = Visibility::Visible;
    settle(&mut a);
    a.app.world_mut().resource_mut::<CooldownMap>().0.clear();
    put_on_bar_state(&mut a);
    a.app.world_mut().resource_mut::<Messages<GameEvent>>().clear();
    a.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit8);
    a.app.update();
    a.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
    assert!(has(&events(&a.app), "item.used:potion"), "a hotkey fires in the mode: {:?}", events(&a.app));
    assert!(mode(&a).is_some(), "and the mode stays on");
}

fn put_on_bar_state(a: &mut AssignApp) {
    a.app.world_mut().resource_mut::<LoadedInventoryUi>().assign_mode = Some(AssignMode { item: "potion".to_string(), from: None });
    settle(a);
}

#[test]
fn the_mode_cancels_from_state_and_from_the_gestures_that_cancel_it() {
    use ironhold_core::runtime::sim_clock::{PauseHold, SimClock};
    // a right click anywhere
    let mut a = assign_app(&[("potion", 3)]);
    put_on_bar(&mut a);
    assert!(mode(&a).is_some());
    a.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    a.app.update();
    a.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
    settle(&mut a);
    assert_eq!(mode(&a), None, "right click cancels");
    assert!(a.app.world().resource::<ActionBarAssignments>().is_empty());
    // [Cancel]
    put_on_bar(&mut a);
    let cancel = strip_button(&mut a, AssignStripButton::Cancel);
    let _ = press_entity(&mut a, cancel);
    settle(&mut a);
    assert_eq!(mode(&a), None, "Cancel cancels");
    // choosing Put on bar again cancels
    put_on_bar(&mut a);
    assert!(mode(&a).is_some());
    put_on_bar(&mut a);
    settle(&mut a);
    assert_eq!(mode(&a), None, "choosing the row again cancels");
    // closing the bag
    put_on_bar(&mut a);
    *a.app.world_mut().get_mut::<Visibility>(a.panel).unwrap() = Visibility::Hidden;
    settle(&mut a);
    assert_eq!(mode(&a), None, "closing the bag cancels");
    *a.app.world_mut().get_mut::<Visibility>(a.panel).unwrap() = Visibility::Visible;
    settle(&mut a);
    // the pause menu
    put_on_bar(&mut a);
    a.app.world_mut().resource_mut::<SimClock>().hold(PauseHold::Menu);
    settle(&mut a);
    assert_eq!(mode(&a), None, "a pause cancels");
    a.app.world_mut().resource_mut::<SimClock>().release(PauseHold::Menu);
    settle(&mut a);
    // the item leaves the bag
    put_on_bar(&mut a);
    let _ = run(&mut a.app, vec![Action::RemoveItem { entity: "player".to_string(), item_key: "potion".to_string(), count: 3, all_or_nothing: false }]);
    settle(&mut a);
    assert_eq!(mode(&a), None, "an item put on the bar from the bag cancels when it leaves the bag");
}

#[test]
fn a_click_on_another_slot_cancels_without_firing_it_and_a_world_click_does_not_cancel() {
    let mut a = assign_app(&[("potion", 3)]);
    put_on_bar(&mut a);
    let s1 = a.s1;
    let ev = press_entity(&mut a, s1);
    assert!(!has(&ev, "plain.fired"), "the click was swallowed by the mode: {ev:?}");
    settle(&mut a);
    assert_eq!(mode(&a), None, "and it cancelled");
    // a press that lands on nothing (the world) does not cancel
    put_on_bar(&mut a);
    a.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    a.app.update();
    a.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
    settle(&mut a);
    assert!(mode(&a).is_some(), "a world click is not a cancel");
}

#[test]
fn a_rule_on_assign_slot_replaces_the_built_in_but_still_ends_the_mode() {
    let mut a = assign_app(&[("potion", 3)]);
    a.app.world_mut().insert_resource(fsm_with("playing", Some("ui.button_pressed:assign_slot:7"), false));
    a.app.world_mut().resource_mut::<LogicState>().0 = "playing".to_string();
    a.app.update();
    put_on_bar(&mut a);
    let s7 = a.s7;
    let ev = press_entity(&mut a, s7);
    assert!(has(&ev, "rule.ran"), "{ev:?}");
    settle(&mut a);
    assert_eq!(slot_item(&a, a.s7), None, "the rule owns the event: nothing was assigned");
    assert_eq!(mode(&a), None, "the mode ended anyway");
    assert!(a.app.world().resource::<LoadedInventoryUi>().assign_handoff.is_empty(), "no stale handoff");
}

#[test]
fn remove_from_bar_in_the_bag_menu_empties_every_slot_holding_the_item() {
    let mut a = assign_app(&[("potion", 3)]);
    for slot in ["7", "8"] {
        let _ = run(&mut a.app, vec![Action::AssignBarSlot { bar: "main".to_string(), slot: slot.to_string(), item_key: "potion".to_string() }]);
    }
    settle(&mut a);
    let first = a.bag[0];
    let _ = press_entity(&mut a, first);
    assert!(row_shown(&mut a.app, MenuRowKind::RemoveFromBar), "the item is on two slots");
    let text = {
        let mut q = a.app.world_mut().query::<(&ironhold_core::capabilities::slot_menu::SlotMenuRowText, &Text)>();
        q.iter(a.app.world()).find(|(m, _)| m.0 == MenuRowKind::RemoveFromBar).map(|(_, t)| t.0.clone()).unwrap()
    };
    assert_eq!(text, "Remove from bar (2 slots)");
    let row = row_entity(&mut a.app, MenuRowKind::RemoveFromBar);
    let _ = press_entity(&mut a, row);
    settle(&mut a);
    assert_eq!(slot_item(&a, a.s7), None);
    assert_eq!(slot_item(&a, a.s8), None);
}

#[test]
fn a_right_click_on_a_hovered_bag_item_in_assign_mode_cancels_without_using_it() {
    let mut a = assign_app(&[("potion", 3)]);
    put_on_bar(&mut a);
    assert!(mode(&a).is_some());
    *a.app.world_mut().get_mut::<Interaction>(a.bag[0]).unwrap() = Interaction::Hovered;
    a.app.update();
    a.app.world_mut().resource_mut::<Messages<GameEvent>>().clear();
    a.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    a.app.update();
    let ev = events(&a.app);
    a.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
    assert!(!has(&ev, "item.used:potion"), "the right click only cancels: {ev:?}");
    settle(&mut a);
    assert_eq!(mode(&a), None);
    assert_eq!(bag_count(&a.app, "potion"), 3, "nothing was drunk");
}

#[test]
fn a_no_item_press_also_names_the_item_on_an_assignable_slot() {
    let mut a = assign_app(&[]);
    let _ = run(&mut a.app, vec![Action::AssignBarSlot { bar: "main".to_string(), slot: "8".to_string(), item_key: "potion".to_string() }]);
    settle(&mut a);
    *a.app.world_mut().get_mut::<Visibility>(a.panel).unwrap() = Visibility::Hidden;
    settle(&mut a);
    let s8 = a.s8;
    let ev = press_entity(&mut a, s8);
    assert!(has(&ev, "action_bar.no_item:8") && has(&ev, "action_bar.no_item:8:potion"), "both forms: {ev:?}");
}

#[test]
fn an_assignable_slot_with_authored_do_actions_never_runs_them_while_empty() {
    // The loader bakes none into an assignable slot: even if a slot entity somehow carries do_actions, emptiness refuses before the intent.
    let mut a = assign_app(&[]);
    a.app.world_mut().get_mut::<ActionSlotUi>(a.s7).unwrap().do_actions = vec![emit_action("dead.code")];
    let s7 = a.s7;
    let ev = press_entity(&mut a, s7);
    assert!(has(&ev, "action_bar.empty_slot:7") && !has(&ev, "dead.code"), "{ev:?}");
}

// Item system S2a (`planning/features/item_transaction_primitive.md`): buy, transfer, add (coins) and remove go through
// one all-or-nothing transaction, merchant stock is runtime state, and every outcome has a defined event.
//
// No test covered BuyItem / TransferItem / AddItem / RemoveItem before this file (only TakeAllFromContainer, in
// corpse_loot_interact_tests.rs), so these tests are the regression net for the item actions.
use bevy::prelude::*;
use ironhold_core::capabilities::inventory::{
    add_to_slots, GoldLabelMarker, Inventory, InventorySlotMarker, LoadedInventoryUi, LoadedItemCatalog, PanelKind, PlayerInventory,
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
            stock: vec![ShopEntry { item_key: "old_key".to_string(), buy_price: 50, sell_price: 0, stock_count: stock }],
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
fn a_left_click_on_a_slot_uses_its_item() {
    let mut app = use_app(&[("potion", 2)]);
    let s = slot(&mut app, 0);
    app.update();
    app.world_mut().resource_mut::<Messages<GameEvent>>().clear();
    *app.world_mut().get_mut::<Interaction>(s).unwrap() = Interaction::Pressed;
    app.update();
    assert!(has(&events(&app), "item.used:potion"), "{:?}", events(&app));
    assert_eq!(bag_count(&app, "potion"), 1);
    // The slot is still `Pressed` (the mouse button is held): only the edge counts, so holding it must not drink again.
    app.world_mut().resource_mut::<Messages<GameEvent>>().clear();
    app.update();
    app.update();
    assert!(!has(&events(&app), "item.used:potion"), "holding the button must not keep using the item: {:?}", events(&app));
    assert_eq!(bag_count(&app, "potion"), 1);
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

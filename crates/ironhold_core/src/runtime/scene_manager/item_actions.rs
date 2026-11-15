//! Executor-side glue for the item actions that go through the transaction primitive
//! (`capabilities/item_txn.rs`, item system S2a, `planning/features/done/item_transaction_primitive.md`).
//!
//! The pure primitive works on an owned `TxnLedger`; this file gathers that ledger from the ECS (clones of the touched
//! slot vectors, stat values and stock entries), calls `plan`, and on success writes each touched entry back one at a
//! time. Nothing here holds two `&mut` ECS borrows across the call. Events and the built-in messages are emitted here so
//! every failure has one defined shape and a failure never emits a success event.

use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::capabilities::inventory::{MerchantStockState, PanelKind};
use crate::capabilities::item_txn::{
    count_item, plan, resolve_stat_source, room_for, sell_price_for, InvKey, InvLedger, StatLedger, StatOwner, StatRef, TxnError,
    TxnLedger, TxnOp,
};
use crate::runtime::actions::ActionQueue;
use crate::runtime::messages::GameEvent;
use crate::schema::catalog::MerchantDef;
use crate::schema::ui::UiAction;
use crate::schema::Action;

use super::action_substitution::{rewrite_self, rewrite_target};
use super::{SceneStateParams, SpawnParams};

/// `"player"` addresses `PlayerInventory`, anything else a container entity's spawn id.
fn inv_key(entity: &str) -> InvKey {
    if entity == "player" { InvKey::Player } else { InvKey::Entity(entity.to_string()) }
}

fn load_inv(scene_state: &mut SceneStateParams, key: &InvKey) -> Option<InvLedger> {
    match key {
        InvKey::Player => {
            let p = &*scene_state.player_inventory;
            Some(InvLedger { slots: p.slots.clone(), max_slots: p.max_slots })
        }
        InvKey::Entity(id) => scene_state
            .container_inventories
            .iter()
            .find(|(sid, _)| &sid.0 == id)
            .map(|(_, inv)| InvLedger { slots: inv.slots.clone(), max_slots: inv.max_slots }),
    }
}

fn store_inv(scene_state: &mut SceneStateParams, key: &InvKey, ledger: &InvLedger) {
    match key {
        InvKey::Player => scene_state.player_inventory.slots = ledger.slots.clone(),
        InvKey::Entity(id) => {
            for (sid, mut inv) in scene_state.container_inventories.iter_mut() {
                if &sid.0 == id {
                    inv.slots = ledger.slots.clone();
                    break;
                }
            }
        }
    }
}

/// The player every "the player" lookup means: the one with the lowest `PlayerIndex` (a player without an index
/// sorts last). `PlayerInventory.player_spawn_id` is only set by the dynamic spawn arm, so it is only a fallback.
pub(super) fn primary_player_id(scene_state: &SceneStateParams) -> Option<String> {
    crate::capabilities::item_txn::pick_primary_player(
        scene_state.primary_players.iter().map(|(sid, idx)| (sid.0.as_str(), idx.map(|i| i.0))),
        scene_state.player_inventory.player_spawn_id.as_deref(),
    )
    .map(str::to_string)
}

/// Where a currency stat lives and its bounds. One rule everywhere (`resolve_stat_source`): the player's own
/// `StatMap` first, then the global pool; `None` when neither has the stat.
pub(super) fn resolve_currency(
    scene_state: &SceneStateParams,
    stat_key: &str,
    player_id: Option<&str>,
) -> Option<(StatRef, StatLedger)> {
    let player_stats = player_id.and_then(|pid| {
        scene_state.stat_map_query.iter().find(|(sid, _)| sid.0 == pid).map(|(_, sm)| sm)
    });
    let loaded = Some(&*scene_state.loaded_stats);
    let (_, own) = resolve_stat_source(stat_key, player_stats, loaded);
    if own {
        let live = player_stats?.0.get(stat_key)?;
        let pid = player_id?;
        return Some((
            StatRef { owner: StatOwner::Entity(pid.to_string()), key: stat_key.to_string() },
            StatLedger { current: live.current, min: live.def.min, max: live.def.max },
        ));
    }
    let live = scene_state.loaded_stats.0.get(stat_key)?;
    Some((
        StatRef { owner: StatOwner::Global, key: stat_key.to_string() },
        StatLedger { current: live.current, min: live.def.min, max: live.def.max },
    ))
}

/// Writes a planned stat value back through `apply_delta` (so a drop still resets the stat's regen cooldown).
pub(super) fn store_stat(scene_state: &mut SceneStateParams, stat: &StatRef, old: f32, new: f32) {
    let delta = new - old;
    if delta == 0.0 {
        return;
    }
    match &stat.owner {
        StatOwner::Global => {
            if let Some(live) = scene_state.loaded_stats.0.get_mut(stat.key.as_str()) {
                live.apply_delta(delta);
            }
        }
        StatOwner::Entity(id) => {
            for (sid, mut sm) in scene_state.stat_map_query.iter_mut() {
                if &sid.0 == id {
                    if let Some(live) = sm.0.get_mut(stat.key.as_str()) {
                        live.apply_delta(delta);
                    }
                    break;
                }
            }
        }
    }
}

/// How many whole units of room a currency stat has before its max.
pub(super) fn stat_room(stat: &StatLedger) -> u32 {
    (stat.max - stat.current).max(0.0).floor() as u32
}

/// Remaining stock per item for one merchant, seeded from its def on first use. Stock is runtime state keyed by the
/// merchant entity: a new entity (scene load, respawn) starts full, and `LoadScene` clears the whole map.
pub(super) fn merchant_stock_for(
    state: &mut MerchantStockState,
    merchant: Option<Entity>,
    def: &MerchantDef,
) -> BTreeMap<String, Option<u32>> {
    let seed = || def.stock.iter().map(|e| (e.item_key.clone(), e.stock_count)).collect::<BTreeMap<_, _>>();
    match merchant {
        Some(e) => state.0.entry(e).or_insert_with(seed).clone(),
        None => seed(),
    }
}

fn say(action_queue: &mut ActionQueue, player_id: &Option<String>, text: String) {
    if let Some(pid) = player_id {
        action_queue.push(Action::ShowFloatingText {
            entity: pid.clone(),
            text,
            offset: Some((0.0, 2.2, 0.0)),
        });
    }
}

fn emit(game_events: &mut MessageWriter<GameEvent>, name: String) {
    game_events.write(GameEvent::Trigger(name));
}

// ─── BuyItem ──────────────────────────────────────────────────────────────────

pub(super) fn buy_item(
    item_key: String,
    commands: &mut Commands,
    scene_state: &mut SceneStateParams,
    spawn_params: &SpawnParams,
    game_events: &mut MessageWriter<GameEvent>,
    action_queue: &mut ActionQueue,
) {
    let Some(merchant_id) = scene_state.inventory_ui.active_merchant_id.clone() else {
        warn!("Action::BuyItem: no shop is currently open");
        return;
    };
    let merchant_entity = spawn_params.registry.entities.get(&merchant_id).copied();
    let merchant_def = merchant_entity
        .and_then(|e| scene_state.prefab_keys.get(e).ok().map(|k| k.0.clone()))
        .and_then(|key| spawn_params.prefab_catalog.0.prefabs.get(&key).and_then(|p| p.merchant.clone()));
    let Some(merchant_def) = merchant_def else {
        warn!("Action::BuyItem: active merchant '{}' has no MerchantDef", merchant_id);
        return;
    };
    let Some(entry) = merchant_def.stock.iter().find(|e| e.item_key == item_key) else {
        warn!("Action::BuyItem: item '{}' not in merchant '{}' stock", item_key, merchant_id);
        return;
    };

    let display_name = scene_state
        .loaded_item_catalog
        .0
        .as_ref()
        .and_then(|c| c.items.get(&item_key))
        .map(|d| d.display_name.clone())
        .unwrap_or_else(|| item_key.clone());
    let player_id = primary_player_id(scene_state);
    let currency = merchant_def.currency_stat.clone();
    let price = entry.buy_price as f32;

    // Gather the ledger: the player's bag, the currency stat and this merchant's remaining stock.
    let mut ledger = TxnLedger::default();
    let Some(bag) = load_inv(scene_state, &InvKey::Player) else { return };
    ledger.invs.insert(InvKey::Player, bag);
    let stock = merchant_stock_for(&mut scene_state.merchant_stock, merchant_entity, &merchant_def);
    ledger.stock.insert(item_key.clone(), stock.get(&item_key).copied().unwrap_or(entry.stock_count));

    // Stock first, then gold, then the item: a broke player clicking a sold-out item hears "sold out".
    let mut ops = vec![TxnOp::Stock { item: item_key.clone(), delta: -1 }];
    let stat = resolve_currency(scene_state, &currency, player_id.as_deref());
    let stat_ref = match &stat {
        Some((r, l)) => {
            ledger.stats.insert(r.clone(), *l);
            r.clone()
        }
        None => {
            // No such currency stat anywhere: nothing can be afforded (a zero price still can).
            let r = StatRef { owner: StatOwner::Global, key: currency.clone() };
            ledger.stats.insert(r.clone(), StatLedger { current: 0.0, min: 0.0, max: 0.0 });
            r
        }
    };
    if price > 0.0 {
        ops.push(TxnOp::Stat { stat: stat_ref.clone(), delta: -price });
    }
    ops.push(TxnOp::Give { inv: InvKey::Player, item: item_key.clone(), count: 1 });

    let result = plan(&ops, &ledger, scene_state.loaded_item_catalog.0.as_ref());
    match result {
        Ok(done) => {
            if let Some(bag) = done.invs.get(&InvKey::Player) {
                store_inv(scene_state, &InvKey::Player, bag);
            }
            if let (Some((r, old)), Some(new)) = (stat.as_ref().map(|(r, l)| (r, l.current)), done.stats.get(&stat_ref)) {
                store_stat(scene_state, r, old, new.current);
            }
            if let (Some(e), Some(rem)) = (merchant_entity, done.stock.get(&item_key)) {
                if let Some(map) = scene_state.merchant_stock.0.get_mut(&e) {
                    map.insert(item_key.clone(), *rem);
                }
            }
            info!("Action::BuyItem: bought '{}' for {} {}", item_key, price, currency);
            emit(game_events, format!("item.bought:{}", item_key));
            say(action_queue, &player_id, format!("Bought {}!", display_name));
            // Refresh the rows in place so the remaining-stock label is current, but only while this merchant's shop
            // is still the open one: a rule such as `[BuyItem(x), CloseShop]` or a `LoadScene` queued behind the buy
            // must not see the shop re-opened.
            if scene_state.inventory_ui.is_panel_open(PanelKind::Shop)
                && scene_state.inventory_ui.active_merchant_id.as_deref() == Some(merchant_id.as_str())
            {
                let shop = scene_state.shop_panel_q.iter().next().map(|(e, _, marker)| (e, marker.font_size));
                let entries = shop.and_then(|(shop_entity, _)| {
                    scene_state.shop_entries_q.iter().find(|(_, c)| c.parent() == shop_entity).map(|(e, _)| e)
                });
                if let (Some((_, font_size)), Some(entries_entity)) = (shop, entries) {
                    rebuild_shop_rows(commands, scene_state, entries_entity, merchant_entity, &merchant_def, font_size);
                }
            }
        }
        Err(TxnError::SoldOut) => {
            info!("Action::BuyItem: item '{}' is sold out", item_key);
            emit(game_events, "shop.sold_out".to_string());
            emit(game_events, format!("shop.sold_out:{}", item_key));
            say(action_queue, &player_id, "Sold out!".to_string());
        }
        Err(TxnError::InsufficientFunds) => {
            info!("Action::BuyItem: not enough {} for '{}'", currency, item_key);
            emit(game_events, "shop.insufficient_funds".to_string());
            emit(game_events, format!("shop.insufficient_funds:{}", item_key));
            say(action_queue, &player_id, format!("Not enough {}!", currency));
        }
        Err(TxnError::NoRoom) => {
            info!("Action::BuyItem: no room for '{}'", item_key);
            emit(game_events, "shop.no_room".to_string());
            emit(game_events, format!("shop.no_room:{}", item_key));
            emit(game_events, "inventory.full:player".to_string());
            say(action_queue, &player_id, "Bag is full!".to_string());
        }
        Err(e) => warn!("Action::BuyItem: '{}' could not be bought ({:?})", item_key, e),
    }
}

// ─── TransferItem ─────────────────────────────────────────────────────────────

pub(super) fn transfer_item(
    from: String,
    to: String,
    item_key: String,
    count: u32,
    scene_state: &mut SceneStateParams,
    game_events: &mut MessageWriter<GameEvent>,
) {
    let failed = format!("inventory.transfer_failed:{}:{}:{}", from, to, item_key);
    if from == to {
        warn!("Action::TransferItem: '{}' is both source and destination", from);
        emit(game_events, failed);
        return;
    }
    let (from_key, to_key) = (inv_key(&from), inv_key(&to));
    let (Some(src), Some(dst)) = (load_inv(scene_state, &from_key), load_inv(scene_state, &to_key)) else {
        warn!("Action::TransferItem: '{}' or '{}' not found or has no Inventory", from, to);
        emit(game_events, failed);
        return;
    };
    let catalog = scene_state.loaded_item_catalog.0.clone();
    let want = count.min(count_item(&src.slots, &item_key));

    // A coin item sent to the player credits its currency stat instead of using a slot.
    let player_id = primary_player_id(scene_state);
    let coin_key = if to_key == InvKey::Player {
        catalog.as_ref().and_then(|c| c.items.get(&item_key)).and_then(|d| d.currency_stat.clone())
    } else {
        None
    };
    let coin_stat = coin_key.as_ref().and_then(|k| resolve_currency(scene_state, k, player_id.as_deref()));
    if let (Some(k), None) = (&coin_key, &coin_stat) {
        warn!("Action::TransferItem: currency_stat {:?} not found in stats; '{}' is moved as an ordinary item", k, item_key);
    }

    let moved = match &coin_stat {
        Some((_, sl)) => want.min(stat_room(sl)),
        None => room_for(&dst, &item_key, want, catalog.as_ref()).min(want),
    };

    if moved == 0 {
        info!("Action::TransferItem: nothing of '{}' moved from '{}' to '{}'", item_key, from, to);
        if want > 0 {
            emit(game_events, format!("inventory.full:{}", to));
        }
        emit(game_events, failed);
        return;
    }

    let mut ledger = TxnLedger::default();
    ledger.invs.insert(from_key.clone(), src);
    ledger.invs.insert(to_key.clone(), dst);
    let mut ops = vec![TxnOp::Take { inv: from_key.clone(), item: item_key.clone(), count: moved }];
    match &coin_stat {
        Some((r, sl)) => {
            ledger.stats.insert(r.clone(), *sl);
            ops.push(TxnOp::Stat { stat: r.clone(), delta: moved as f32 });
        }
        None => ops.push(TxnOp::Give { inv: to_key.clone(), item: item_key.clone(), count: moved }),
    }
    let Ok(done) = plan(&ops, &ledger, catalog.as_ref()) else {
        warn!("Action::TransferItem: could not move '{}' from '{}' to '{}'", item_key, from, to);
        emit(game_events, failed);
        return;
    };
    store_inv(scene_state, &from_key, &done.invs[&from_key]);
    match &coin_stat {
        Some((r, sl)) => store_stat(scene_state, r, sl.current, done.stats[r].current),
        None => store_inv(scene_state, &to_key, &done.invs[&to_key]),
    }
    if coin_stat.is_some() {
        emit(game_events, format!("inventory.added:player:{}:{}", item_key, moved));
    }
    emit(game_events, format!("inventory.transferred:{}:{}:{}", from, to, item_key));
    if moved < want {
        emit(game_events, format!("inventory.full:{}", to));
    }
}

// ─── RemoveItem ───────────────────────────────────────────────────────────────

pub(super) fn remove_item(
    entity: String,
    item_key: String,
    count: u32,
    all_or_nothing: bool,
    scene_state: &mut SceneStateParams,
    game_events: &mut MessageWriter<GameEvent>,
) {
    let key = inv_key(&entity);
    let Some(inv) = load_inv(scene_state, &key) else {
        warn!("Action::RemoveItem: entity '{}' not found or has no Inventory", entity);
        // A mistyped spawn id must not make a guarded turn-in silently never fire.
        emit(game_events, format!("inventory.remove_failed:{}", entity));
        emit(game_events, format!("inventory.remove_failed:{}:{}", entity, item_key));
        return;
    };
    let have = count_item(&inv.slots, &item_key);
    let take = if all_or_nothing { count } else { count.min(have) };
    let mut ledger = TxnLedger::default();
    ledger.invs.insert(key.clone(), inv);
    let ops = [TxnOp::Take { inv: key.clone(), item: item_key.clone(), count: take }];
    match plan(&ops, &ledger, None) {
        Ok(done) if take > 0 => {
            store_inv(scene_state, &key, &done.invs[&key]);
            emit(game_events, format!("inventory.removed:{}:{}:{}", entity, item_key, take));
        }
        _ => {
            // Nothing removed (none held, or all_or_nothing and fewer than `count`): a failure, never a success event.
            emit(game_events, format!("inventory.remove_failed:{}", entity));
            emit(game_events, format!("inventory.remove_failed:{}:{}", entity, item_key));
        }
    }
}

// ─── AddItem (coin routing only; a normal add keeps its partial-add contract) ─

/// A coin item given to the player credits its currency stat instead of using a slot. `Some(credited)` when the item
/// is a coin whose stat exists (the caller then emits the events), `None` for an ordinary item or a coin whose stat
/// is missing (that coin stays an ordinary item, with a warning).
pub(super) fn credit_coins(scene_state: &mut SceneStateParams, item_key: &str, count: u32) -> Option<u32> {
    let stat_key = scene_state
        .loaded_item_catalog
        .0
        .as_ref()
        .and_then(|c| c.items.get(item_key))
        .and_then(|d| d.currency_stat.clone())?;
    let player_id = primary_player_id(scene_state);
    let Some((r, sl)) = resolve_currency(scene_state, &stat_key, player_id.as_deref()) else {
        warn!("Action::AddItem: currency_stat {:?} not found in stats; '{}' is added as an ordinary item", stat_key, item_key);
        return None;
    };
    let credited = count.min(stat_room(&sl));
    store_stat(scene_state, &r, sl.current, sl.current + credited as f32);
    Some(credited)
}

/// Rebuilds the shop panel's item rows from the merchant's `MerchantDef` and its remaining stock. Used by `OpenShop`
/// and, in place, after a successful buy (a queued `OpenShop` would run after anything already queued behind the buy,
/// and could re-open a shop a designer's rule had just closed).
pub(super) fn rebuild_shop_rows(
    commands: &mut Commands,
    scene_state: &mut SceneStateParams,
    entries_entity: Entity,
    entity_opt: Option<Entity>,
    merchant_def: &MerchantDef,
    font_size: f32,
) {
    // Remaining stock lives in runtime state (`MerchantStockState`), never in the prefab catalog.
    let stock_now = super::item_actions::merchant_stock_for(
        &mut scene_state.merchant_stock, entity_opt, &merchant_def,
    );
    let catalog = &scene_state.loaded_item_catalog.0;
    let panel_icon_sheet = scene_state.inventory_ui.panel_icon_sheet.clone();
    commands.entity(entries_entity).despawn_children();
    for entry in &merchant_def.stock {
        let display_name = catalog
            .as_ref()
            .and_then(|c| c.items.get(&entry.item_key))
            .map(|d| d.display_name.clone())
            .unwrap_or_else(|| entry.item_key.clone());
        let remaining = stock_now.get(&entry.item_key).copied().unwrap_or(entry.stock_count);
        let sold_out = remaining == Some(0);
        let stock_label = match remaining {
            Some(0) => " (Sold out)".to_string(),
            Some(n) => format!(" [{}]", n),
            None => String::new(),
        };

        // Resolve icon data for this entry (smaller than inventory slots).
        let item_def = catalog.as_ref().and_then(|c| c.items.get(&entry.item_key));
        let icon_index = item_def.map(|d| d.icon_index as usize).unwrap_or(0);
        let icon_color = item_def
            .and_then(|d| d.icon_color)
            .map(|(r, g, b, a)| Color::srgba(r, g, b, a))
            .unwrap_or(Color::WHITE);
        let sheet_key: Option<String> = item_def
            .and_then(|d| d.icon_sheet.clone())
            .or_else(|| panel_icon_sheet.clone());
        // Clone handles so the borrow on inventory_ui ends before with_children.
        let atlas_pair: Option<(Handle<Image>, Handle<TextureAtlasLayout>)> = sheet_key
            .as_deref()
            .and_then(|k| scene_state.inventory_ui.icon_atlases.get(k))
            .map(|(t, l)| (t.clone(), l.clone()));

        let item_key = entry.item_key.clone();
        let buy_price = entry.buy_price;
        commands.entity(entries_entity).with_children(|p| {
            p.spawn((
                Name::new("ShopEntryRow"),
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)),
                    ..default()
                },
            )).with_children(|row| {
                // Item icon (28×28, smaller than inventory slots).
                if let Some((tex, layout)) = atlas_pair {
                    row.spawn((
                        Name::new("ShopEntryIcon"),
                        Node {
                            width: Val::Px(28.0),
                            height: Val::Px(28.0),
                            flex_shrink: 0.0,
                            margin: UiRect::right(Val::Px(6.0)),
                            ..default()
                        },
                        ImageNode {
                            image: tex,
                            texture_atlas: Some(TextureAtlas { layout, index: icon_index }),
                            color: icon_color,
                            ..default()
                        },
                    ));
                }
                // Item name + optional stock count.
                row.spawn((
                    Name::new("ShopEntryName"),
                    Node { flex_grow: 1.0, ..default() },
                    Text::new(format!("{}{}", display_name, stock_label)),
                    TextFont { font_size, ..default() },
                    TextColor(Color::srgba(0.90, 0.88, 0.78, 1.0)),
                ));
                // Buy price (gold tint).
                row.spawn((
                    Name::new("ShopEntryPrice"),
                    Node { margin: UiRect::axes(Val::Px(8.0), Val::Px(0.0)), ..default() },
                    Text::new(format!("{} g", buy_price)),
                    TextFont { font_size, ..default() },
                    TextColor(Color::srgba(0.95, 0.85, 0.40, 1.0)),
                ));
                // Buy button (bigger padding, full font_size).
                row.spawn((
                    Name::new("ShopBuyBtn"),
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    // A sold-out row stays in the list with a dimmed button, and stays clickable so a click still emits the sold-out event.
                    BorderColor::from(if sold_out { Color::srgba(0.35, 0.35, 0.35, 0.8) } else { Color::srgba(0.2, 0.5, 0.2, 0.8) }),
                    BackgroundColor(if sold_out { Color::srgba(0.14, 0.14, 0.14, 0.85) } else { Color::srgba(0.08, 0.20, 0.08, 0.85) }),
                    UiAction::Trigger(format!("buy_item:{}", item_key)),
                )).with_children(|b| {
                    b.spawn((
                        Name::new("BuyBtnText"),
                        Text::new("Buy"),
                        TextFont { font_size, ..default() },
                        TextColor(if sold_out { Color::srgba(0.55, 0.55, 0.55, 1.0) } else { Color::srgba(0.60, 0.90, 0.60, 1.0) }),
                    ));
                });
            });
        });
    }
}

// ─── UseItem ──────────────────────────────────────────────────────────────────

/// Uses one of `item_key` from the player's bag: takes `use.consume` units (none for `consume: 0`, which only needs one
/// held) and, only if that worked, runs the item's `use.do_actions` with `{self}` as the primary player. The effects are
/// inserted at the FRONT of the queue (`ActionQueue::push_front_all`, executor-only), so `[UseItem(x), LoadScene(..)]` runs
/// them before the load. A failure changes nothing, runs nothing and emits `item.use_failed` (+ `:{item}`), never `item.used`.
pub(super) fn use_item(
    item_key: String,
    scene_state: &mut SceneStateParams,
    game_events: &mut MessageWriter<GameEvent>,
    action_queue: &mut ActionQueue,
) {
    let fail = |game_events: &mut MessageWriter<GameEvent>, item_key: &str| {
        emit(game_events, "item.use_failed".to_string());
        emit(game_events, format!("item.use_failed:{}", item_key));
    };

    let use_def = scene_state
        .loaded_item_catalog
        .0
        .as_ref()
        .and_then(|c| c.items.get(&item_key))
        .and_then(|d| d.use_def.clone());
    let Some(use_def) = use_def else {
        info!("Action::UseItem: '{}' is unknown or has no `use:` block", item_key);
        fail(game_events, &item_key);
        return;
    };

    // A `UseItem` inside a use list could loop forever inside one executor drain: never run it.
    let actions: Vec<Action> = use_def
        .do_actions
        .iter()
        .filter(|a| {
            let nested = matches!(a, Action::UseItem(_));
            if nested {
                warn!("Action::UseItem: '{}' has a UseItem inside its `use:` list; it is skipped (it could loop forever)", item_key);
            }
            !nested
        })
        .cloned()
        .collect();

    // `{target}` is not available in an item's actions (nothing fills it in): say so instead of silently doing nothing.
    // Compared through `rewrite_target` so only fields that really are substituted count (an exhaustive match, not a string search).
    if actions.iter().any(|a| rewrite_target(a.clone(), "\u{0}") != *a) {
        warn!("Action::UseItem: '{}' uses {{target}} in its `use:` actions, which is not available there; it stays unreplaced", item_key);
    }

    // `{self}` is the primary player: without one, fail before anything is taken. Only a field `rewrite_self` really
    // substitutes counts (a `Log` text or a sound key that merely contains the token does not).
    let player_id = primary_player_id(scene_state);
    if player_id.is_none() && actions.iter().any(|a| rewrite_self(a.clone(), "\u{0}") != *a) {
        warn!("Action::UseItem: '{}' uses {{self}} but there is no player in the scene", item_key);
        fail(game_events, &item_key);
        return;
    }

    let Some(bag) = load_inv(scene_state, &InvKey::Player) else { return };
    let held = count_item(&bag.slots, &item_key);
    if use_def.consume == 0 {
        // Reusable: the player must hold one, none is taken.
        if held == 0 {
            fail(game_events, &item_key);
            return;
        }
    } else {
        let mut ledger = TxnLedger::default();
        ledger.invs.insert(InvKey::Player, bag);
        let ops = [TxnOp::Take { inv: InvKey::Player, item: item_key.clone(), count: use_def.consume }];
        match plan(&ops, &ledger, None) {
            Ok(done) => {
                if let Some(bag) = done.invs.get(&InvKey::Player) {
                    store_inv(scene_state, &InvKey::Player, bag);
                }
            }
            Err(_) => {
                fail(game_events, &item_key);
                return;
            }
        }
    }

    emit(game_events, format!("item.used:{}", item_key));
    let rewritten: Vec<Action> = match player_id.as_deref() {
        Some(id) => actions.into_iter().map(|a| rewrite_self(a, id)).collect(),
        None => actions,
    };
    action_queue.push_front_all(rewritten);
}

// ─── SellItem / DestroyItem ───────────────────────────────────────────────────

/// How many units an action means: `all` is every unit held across all stacks (read now, not when the action was
/// queued), otherwise `count`. `0` means "nothing to do" and is refused by the callers.
fn units_for(held: u32, count: u32, all: bool) -> u32 {
    if all { held } else { count }
}

/// Sells `count` (or all) of `item_key` to the open shop, all or nothing. See `Action::SellItem`.
pub(super) fn sell_item(
    item_key: String,
    count: u32,
    all: bool,
    scene_state: &mut SceneStateParams,
    spawn_params: &SpawnParams,
    game_events: &mut MessageWriter<GameEvent>,
    action_queue: &mut ActionQueue,
) {
    let player_id = primary_player_id(scene_state);
    let refuse = |game_events: &mut MessageWriter<GameEvent>, action_queue: &mut ActionQueue, why: &str| {
        emit(game_events, "item.sell_refused".to_string());
        emit(game_events, format!("item.sell_refused:{}", item_key));
        say(action_queue, &player_id, why.to_string());
    };

    let Some(merchant_id) = scene_state.inventory_ui.active_merchant_id.clone() else {
        info!("Action::SellItem: no shop is currently open");
        refuse(game_events, action_queue, "Nobody to sell to here");
        return;
    };
    let merchant_def = spawn_params
        .registry
        .entities
        .get(&merchant_id)
        .and_then(|e| scene_state.prefab_keys.get(*e).ok().map(|k| k.0.clone()))
        .and_then(|key| spawn_params.prefab_catalog.0.prefabs.get(&key).and_then(|p| p.merchant.clone()));
    let Some(merchant_def) = merchant_def else {
        warn!("Action::SellItem: active merchant '{}' has no MerchantDef", merchant_id);
        refuse(game_events, action_queue, "Can't sell that here");
        return;
    };
    let item_def = scene_state.loaded_item_catalog.0.as_ref().and_then(|c| c.items.get(&item_key)).cloned();
    let Some(item_def) = item_def else {
        info!("Action::SellItem: unknown item '{}'", item_key);
        refuse(game_events, action_queue, "Can't sell that here");
        return;
    };
    let Some(price) = sell_price_for(&merchant_def, &item_key, item_def.sell_price) else {
        info!("Action::SellItem: merchant '{}' does not buy '{}'", merchant_id, item_key);
        refuse(game_events, action_queue, "Can't sell that here");
        return;
    };

    let Some(bag) = load_inv(scene_state, &InvKey::Player) else { return };
    let units = units_for(count_item(&bag.slots, &item_key), count, all);
    if units == 0 {
        info!("Action::SellItem: nothing to sell of '{}' (count 0, or none held)", item_key);
        refuse(game_events, action_queue, "You have none to sell");
        return;
    }
    let total = price as u64 * units as u64;

    let currency = merchant_def.currency_stat.clone();
    let Some((stat_ref, stat_ledger)) = resolve_currency(scene_state, &currency, player_id.as_deref()) else {
        warn!("Action::SellItem: currency stat '{}' not found; nothing can be paid out", currency);
        refuse(game_events, action_queue, "Can't sell that here");
        return;
    };
    let mut ledger = TxnLedger::default();
    ledger.invs.insert(InvKey::Player, bag);
    ledger.stats.insert(stat_ref.clone(), stat_ledger);
    let ops = [
        TxnOp::Take { inv: InvKey::Player, item: item_key.clone(), count: units },
        TxnOp::Stat { stat: stat_ref.clone(), delta: total as f32 },
    ];
    match plan(&ops, &ledger, scene_state.loaded_item_catalog.0.as_ref()) {
        Ok(done) => {
            if let Some(bag) = done.invs.get(&InvKey::Player) {
                store_inv(scene_state, &InvKey::Player, bag);
            }
            if let Some(new) = done.stats.get(&stat_ref) {
                store_stat(scene_state, &stat_ref, stat_ledger.current, new.current);
            }
            info!("Action::SellItem: sold {} x '{}' for {} {}", units, item_key, total, currency);
            emit(game_events, format!("item.sold:{}", item_key));
            say(action_queue, &player_id, format!("Sold {} for {} {}", item_def.display_name, total, currency));
        }
        Err(TxnError::NoRoom) => {
            info!("Action::SellItem: paying {} {} would pass the stat's maximum", total, currency);
            refuse(game_events, action_queue, "Can't carry any more");
        }
        Err(e) => {
            info!("Action::SellItem: '{}' could not be sold ({:?})", item_key, e);
            refuse(game_events, action_queue, "You have none to sell");
        }
    }
}

/// Destroys `count` (or all) of `item_key` from the player's bag, all or nothing. See `Action::DestroyItem`.
pub(super) fn destroy_item(
    item_key: String,
    count: u32,
    all: bool,
    scene_state: &mut SceneStateParams,
    game_events: &mut MessageWriter<GameEvent>,
    action_queue: &mut ActionQueue,
) {
    let player_id = primary_player_id(scene_state);
    let refuse = |game_events: &mut MessageWriter<GameEvent>, action_queue: &mut ActionQueue, why: &str| {
        emit(game_events, "item.destroy_refused".to_string());
        emit(game_events, format!("item.destroy_refused:{}", item_key));
        say(action_queue, &player_id, why.to_string());
    };
    let item_def = scene_state.loaded_item_catalog.0.as_ref().and_then(|c| c.items.get(&item_key)).cloned();
    let Some(item_def) = item_def else {
        info!("Action::DestroyItem: unknown item '{}'", item_key);
        refuse(game_events, action_queue, "Can't destroy that");
        return;
    };
    if !item_def.destroyable {
        info!("Action::DestroyItem: '{}' is not destroyable", item_key);
        refuse(game_events, action_queue, "You can't destroy that");
        return;
    }
    let Some(bag) = load_inv(scene_state, &InvKey::Player) else { return };
    let units = units_for(count_item(&bag.slots, &item_key), count, all);
    if units == 0 {
        info!("Action::DestroyItem: nothing to destroy of '{}' (count 0, or none held)", item_key);
        refuse(game_events, action_queue, "You have none to destroy");
        return;
    }
    let mut ledger = TxnLedger::default();
    ledger.invs.insert(InvKey::Player, bag);
    let ops = [TxnOp::Take { inv: InvKey::Player, item: item_key.clone(), count: units }];
    match plan(&ops, &ledger, None) {
        Ok(done) => {
            if let Some(bag) = done.invs.get(&InvKey::Player) {
                store_inv(scene_state, &InvKey::Player, bag);
            }
            info!("Action::DestroyItem: destroyed {} x '{}'", units, item_key);
            emit(game_events, format!("item.destroyed:{}", item_key));
        }
        Err(_) => refuse(game_events, action_queue, "You have none to destroy"),
    }
}

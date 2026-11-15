//! The item transaction primitive (item system S2a, `planning/features/done/item_transaction_primitive.md`).
//!
//! Every action that moves items, gold or shop stock in one step (buy, transfer, an all-or-nothing remove) goes through
//! [`plan`]: it applies the ops, in order, to a *clone* of an owned [`TxnLedger`] and returns the new ledger, or the
//! first [`TxnError`], in which case nothing the caller owns has changed. The executor gathers the ledger from the ECS
//! (clones of the touched slot vectors, stat values and stock entries), calls `plan`, and on success writes each touched
//! entry back one at a time. No ECS type appears in this file, so it is unit-testable without an `App`, and no
//! container with an unstable iteration order is used (`BTreeMap`/`Vec` only), so the result is deterministic.

use std::collections::BTreeMap;

use crate::capabilities::inventory::{add_to_slots, remove_from_slots};
use crate::schema::catalog::MerchantDef;
use crate::schema::items::{ItemCatalog, ItemStack};
use crate::schema::stats::{LoadedStats, StatMap};

/// Which inventory an op touches. `Entity` holds the spawn id of a container entity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum InvKey {
    Player,
    Entity(String),
}

/// Who owns a stat: the global pool (`LoadedStats`) or one entity's own `StatMap` (by spawn id).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StatOwner {
    Global,
    Entity(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StatRef {
    pub owner: StatOwner,
    pub key: String,
}

/// A clone of one inventory's slots.
#[derive(Debug, Clone)]
pub struct InvLedger {
    pub slots: Vec<Option<ItemStack>>,
    pub max_slots: usize,
}

/// A clone of one stat's value and bounds.
#[derive(Debug, Clone, Copy)]
pub struct StatLedger {
    pub current: f32,
    pub min: f32,
    pub max: f32,
}

/// Everything a transaction may touch, owned. Cloning it is cheap (a few dozen slots).
#[derive(Debug, Clone, Default)]
pub struct TxnLedger {
    pub invs: BTreeMap<InvKey, InvLedger>,
    pub stats: BTreeMap<StatRef, StatLedger>,
    /// Remaining stock per item key; `None` = unlimited.
    pub stock: BTreeMap<String, Option<u32>>,
}

#[derive(Debug, Clone)]
pub enum TxnOp {
    /// Must remove exactly `count`.
    Take { inv: InvKey, item: String, count: u32 },
    /// Must fit exactly `count`.
    Give { inv: InvKey, item: String, count: u32 },
    /// A negative delta must not take the stat below its `min`; a positive one must not pass its `max`.
    Stat { stat: StatRef, delta: f32 },
    /// A negative delta must not take finite stock below 0 (unlimited stock is untouched).
    Stock { item: String, delta: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxnError {
    NotEnoughItems,
    NoRoom,
    InsufficientFunds,
    SoldOut,
    /// An item catalog is loaded and does not contain the item.
    UnknownItem,
    /// The op names an inventory, stat or stock entry the ledger does not hold (a caller bug).
    MissingTarget,
}

/// Total count of `item_key` across all slots.
pub fn count_item(slots: &[Option<ItemStack>], item_key: &str) -> u32 {
    slots.iter().flatten().filter(|s| s.item_key == item_key).map(|s| s.count).sum()
}

/// How many of `count` units of `item_key` would fit into `inv` right now (without changing it).
pub fn room_for(inv: &InvLedger, item_key: &str, count: u32, catalog: Option<&ItemCatalog>) -> u32 {
    let mut probe = inv.slots.clone();
    add_to_slots(&mut probe, inv.max_slots, item_key, count, catalog).0
}

/// Apply `ops` in order to a clone of `ledger`. `Ok(new_ledger)` when every op succeeded, otherwise the first error;
/// the input ledger is never modified, so a failed plan leaves nothing to undo.
pub fn plan(ops: &[TxnOp], ledger: &TxnLedger, catalog: Option<&ItemCatalog>) -> Result<TxnLedger, TxnError> {
    let mut l = ledger.clone();
    for op in ops {
        match op {
            TxnOp::Take { inv, item, count } => {
                let inv = l.invs.get_mut(inv).ok_or(TxnError::MissingTarget)?;
                if count_item(&inv.slots, item) < *count {
                    return Err(TxnError::NotEnoughItems);
                }
                remove_from_slots(&mut inv.slots, item, *count);
            }
            TxnOp::Give { inv, item, count } => {
                if let Some(c) = catalog {
                    if !c.items.contains_key(item.as_str()) {
                        return Err(TxnError::UnknownItem);
                    }
                }
                let inv = l.invs.get_mut(inv).ok_or(TxnError::MissingTarget)?;
                let max = inv.max_slots;
                let (added, _) = add_to_slots(&mut inv.slots, max, item, *count, catalog);
                if added < *count {
                    return Err(TxnError::NoRoom);
                }
            }
            TxnOp::Stat { stat, delta } => {
                let s = l.stats.get_mut(stat).ok_or(TxnError::MissingTarget)?;
                let next = s.current + delta;
                if *delta < 0.0 && next < s.min {
                    return Err(TxnError::InsufficientFunds);
                }
                if *delta > 0.0 && next > s.max {
                    return Err(TxnError::NoRoom);
                }
                s.current = next;
            }
            TxnOp::Stock { item, delta } => {
                let remaining = l.stock.get_mut(item.as_str()).ok_or(TxnError::MissingTarget)?;
                if let Some(rem) = remaining {
                    if *delta < 0 {
                        let need = delta.unsigned_abs();
                        if *rem < need {
                            return Err(TxnError::SoldOut);
                        }
                        *rem -= need;
                    } else {
                        *rem += *delta as u32;
                    }
                }
            }
        }
    }
    Ok(l)
}

/// The price per unit a merchant pays for `item_key`, or `None` when it does not buy it: the merchant's own
/// `ShopEntry.sell_price` for the item wins when set (`0` = this merchant does not buy it), otherwise the item's own
/// `sell_price` (`None` or `0` = not sellable).
pub fn sell_price_for(merchant: &MerchantDef, item_key: &str, item_price: Option<u32>) -> Option<u32> {
    let own = merchant.stock.iter().find(|e| e.item_key == item_key).and_then(|e| e.sell_price);
    match own.or(item_price) {
        Some(0) | None => None,
        Some(p) => Some(p),
    }
}

/// Where a currency stat is read from: `(current, is_the_players_own_stat)`. The player's own `StatMap` wins when it
/// has the stat, otherwise the global pool; a stat found nowhere reads as `0.0` and `false`. One rule for the action bar's
/// cost check, shops, loot and the gold footer, so they can never disagree about whose gold it is.
pub fn resolve_stat_source(
    stat: &str,
    player_stats: Option<&StatMap>,
    loaded_stats: Option<&LoadedStats>,
) -> (f32, bool) {
    if let Some(live) = player_stats.and_then(|sm| sm.0.get(stat)) {
        return (live.current, true);
    }
    let current = loaded_stats
        .and_then(|ls| ls.0.get(stat))
        .map(|s| s.current)
        .unwrap_or(0.0);
    (current, false)
}

/// The player every "the player" lookup means in the item actions and the gold footer: the one with the lowest
/// `PlayerIndex`. A player without an index counts as index 0, exactly as `targeting::is_primary_player` and the action
/// bar treat it; ties break by spawn id. `fallback` is
/// `PlayerInventory.player_spawn_id`, which only a dynamic spawn sets, so it applies only when no player entity exists.
pub fn pick_primary_player<'a>(
    players: impl Iterator<Item = (&'a str, Option<u32>)>,
    fallback: Option<&'a str>,
) -> Option<&'a str> {
    players
        .min_by_key(|(id, idx)| (idx.unwrap_or(0), *id))
        .map(|(id, _)| id)
        .or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::items::{ItemCatalog, ItemDef};

    fn stack(key: &str, count: u32) -> Option<ItemStack> {
        Some(ItemStack { item_key: key.to_string(), count })
    }

    fn inv(slots: Vec<Option<ItemStack>>) -> InvLedger {
        let max_slots = slots.len();
        InvLedger { slots, max_slots }
    }

    fn gold() -> StatRef {
        StatRef { owner: StatOwner::Global, key: "gold".to_string() }
    }

    fn ledger(player_slots: Vec<Option<ItemStack>>, gold_now: f32, stock: Option<u32>) -> TxnLedger {
        let mut l = TxnLedger::default();
        l.invs.insert(InvKey::Player, inv(player_slots));
        l.stats.insert(gold(), StatLedger { current: gold_now, min: 0.0, max: 99999.0 });
        l.stock.insert("old_key".to_string(), stock);
        l
    }

    fn buy_ops() -> Vec<TxnOp> {
        vec![
            TxnOp::Stock { item: "old_key".to_string(), delta: -1 },
            TxnOp::Stat { stat: gold(), delta: -50.0 },
            TxnOp::Give { inv: InvKey::Player, item: "old_key".to_string(), count: 1 },
        ]
    }

    #[test]
    fn a_buy_that_succeeds_changes_all_three() {
        let l = ledger(vec![None, None], 100.0, Some(1));
        let out = plan(&buy_ops(), &l, None).unwrap();
        assert_eq!(out.stats[&gold()].current, 50.0);
        assert_eq!(out.stock["old_key"], Some(0));
        assert_eq!(count_item(&out.invs[&InvKey::Player].slots, "old_key"), 1);
    }

    #[test]
    fn a_failed_plan_leaves_the_input_untouched() {
        // Full bag: gold and stock would change before the Give fails, but the input ledger must not.
        let l = ledger(vec![stack("sword", 1), stack("sword", 1)], 100.0, Some(1));
        let err = plan(&buy_ops(), &l, None).unwrap_err();
        assert_eq!(err, TxnError::NoRoom);
        assert_eq!(l.stats[&gold()].current, 100.0);
        assert_eq!(l.stock["old_key"], Some(1));
        assert_eq!(count_item(&l.invs[&InvKey::Player].slots, "old_key"), 0);
    }

    #[test]
    fn sold_out_is_reported_before_insufficient_funds() {
        // A broke player clicking a sold-out item hears "sold out": the Stock op runs first.
        let l = ledger(vec![None], 10.0, Some(0));
        assert_eq!(plan(&buy_ops(), &l, None).unwrap_err(), TxnError::SoldOut);
    }

    #[test]
    fn unlimited_stock_never_runs_out() {
        let l = ledger(vec![None, None, None], 500.0, None);
        let mut out = l;
        for _ in 0..3 {
            out = plan(&buy_ops(), &out, None).unwrap();
        }
        assert_eq!(out.stock["old_key"], None);
        assert_eq!(out.stats[&gold()].current, 350.0);
    }

    #[test]
    fn not_enough_gold_is_insufficient_funds() {
        let l = ledger(vec![None], 49.0, Some(1));
        assert_eq!(plan(&buy_ops(), &l, None).unwrap_err(), TxnError::InsufficientFunds);
    }

    #[test]
    fn take_is_strict() {
        let l = ledger(vec![stack("pelt", 1), None], 0.0, None);
        let ops = [TxnOp::Take { inv: InvKey::Player, item: "pelt".to_string(), count: 3 }];
        assert_eq!(plan(&ops, &l, None).unwrap_err(), TxnError::NotEnoughItems);
        let ops = [TxnOp::Take { inv: InvKey::Player, item: "pelt".to_string(), count: 1 }];
        let out = plan(&ops, &l, None).unwrap();
        assert_eq!(count_item(&out.invs[&InvKey::Player].slots, "pelt"), 0);
    }

    #[test]
    fn a_transaction_can_free_a_slot_and_fill_it() {
        // A full bag: swapping one item for another works because the Take runs before the Give.
        let mut l = TxnLedger::default();
        l.invs.insert(InvKey::Player, inv(vec![stack("a", 1), stack("b", 1)]));
        let ops = [
            TxnOp::Take { inv: InvKey::Player, item: "a".to_string(), count: 1 },
            TxnOp::Give { inv: InvKey::Player, item: "c".to_string(), count: 1 },
        ];
        let out = plan(&ops, &l, None).unwrap();
        let slots = &out.invs[&InvKey::Player].slots;
        assert_eq!((count_item(slots, "a"), count_item(slots, "b"), count_item(slots, "c")), (0, 1, 1));
    }

    #[test]
    fn give_merges_into_an_existing_stack_when_the_bag_is_full() {
        let mut l = TxnLedger::default();
        l.invs.insert(InvKey::Player, inv(vec![stack("potion", 2), stack("sword", 1)]));
        let ops = [TxnOp::Give { inv: InvKey::Player, item: "potion".to_string(), count: 3 }];
        let out = plan(&ops, &l, None).unwrap();
        assert_eq!(count_item(&out.invs[&InvKey::Player].slots, "potion"), 5);
    }

    #[test]
    fn coins_past_the_stat_max_are_refused() {
        let mut l = TxnLedger::default();
        l.stats.insert(gold(), StatLedger { current: 99990.0, min: 0.0, max: 99999.0 });
        let ops = [TxnOp::Stat { stat: gold(), delta: 50.0 }];
        assert_eq!(plan(&ops, &l, None).unwrap_err(), TxnError::NoRoom);
    }

    #[test]
    fn an_unknown_item_is_refused_only_when_a_catalog_is_loaded() {
        let mut l = TxnLedger::default();
        l.invs.insert(InvKey::Player, inv(vec![None]));
        let ops = [TxnOp::Give { inv: InvKey::Player, item: "ghost".to_string(), count: 1 }];
        assert!(plan(&ops, &l, None).is_ok(), "no catalog: today's behaviour, any key is allowed");
        let real = ItemDef {
            display_name: "Real".to_string(), icon_sheet: None, icon_index: 0, icon_color: None,
            stackable: true, max_stack: 99, weight: 1.0, tags: vec![], currency_stat: None, sell_price: None, destroyable: true, use_def: None,
        };
        let catalog = ItemCatalog { schema_version: 1, items: std::iter::once(("real".to_string(), real)).collect() };
        assert_eq!(plan(&ops, &l, Some(&catalog)).unwrap_err(), TxnError::UnknownItem);
    }

    #[test]
    fn a_missing_target_is_reported() {
        let l = TxnLedger::default();
        let ops = [TxnOp::Take { inv: InvKey::Entity("chest".to_string()), item: "x".to_string(), count: 1 }];
        assert_eq!(plan(&ops, &l, None).unwrap_err(), TxnError::MissingTarget);
    }

    #[test]
    fn room_for_counts_what_fits_without_changing_anything() {
        let i = inv(vec![stack("a", 1), None]);
        assert_eq!(room_for(&i, "b", 5, None), 5); // stackable to 99 with no catalog
        let full = inv(vec![stack("a", 1), stack("a", 1)]);
        assert_eq!(room_for(&full, "b", 5, None), 0);
        assert_eq!(i.slots.len(), 2);
    }

    #[test]
    fn the_primary_player_is_the_lowest_index() {
        let players = [("player_02", Some(1)), ("player_01", Some(0)), ("zz_no_index", Some(2))];
        assert_eq!(pick_primary_player(players.iter().copied(), Some("fallback")), Some("player_01"));
        assert_eq!(pick_primary_player(std::iter::empty(), Some("fallback")), Some("fallback"));
        assert_eq!(pick_primary_player(std::iter::empty(), None), None);
        // A player without an index counts as index 0 (like targeting and the action bar); ties break by id.
        let tie = [("b", None), ("a", None)];
        assert_eq!(pick_primary_player(tie.iter().copied(), None), Some("a"));
        let none_vs_one = [("p1", Some(1)), ("no_index", None)];
        assert_eq!(pick_primary_player(none_vs_one.iter().copied(), None), Some("no_index"));
    }

    #[test]
    fn the_players_own_stat_wins_over_the_global_pool() {
        use crate::schema::stats::{LiveStat, StatDef};
        let def = StatDef { base: 0.0, min: 0.0, max: 100.0, soft_max: None, regen_rate: 0.0, regen_delay: 0.0, thresholds: vec![] };
        let mut own = StatMap::default();
        let mut live = LiveStat::new(def.clone());
        live.current = 7.0;
        own.0.insert("gold".to_string(), live);
        let mut global = LoadedStats::default();
        let mut g = LiveStat::new(def);
        g.current = 90.0;
        global.0.insert("gold".to_string(), g);
        assert_eq!(resolve_stat_source("gold", Some(&own), Some(&global)), (7.0, true));
        assert_eq!(resolve_stat_source("gold", None, Some(&global)), (90.0, false));
        assert_eq!(resolve_stat_source("mana", Some(&own), Some(&global)), (0.0, false));
    }
}

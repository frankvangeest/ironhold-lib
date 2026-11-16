use bevy::prelude::*;
use bevy::input::gamepad::{Gamepad, GamepadButton};
use std::collections::{BTreeMap, HashSet};

use crate::schema::actions::Action;
use crate::schema::scene_v2::SlotCost;
use crate::schema::stats::{LoadedStats, StatMap};
use crate::runtime::actions::ActionQueue;
use crate::runtime::messages::GameEvent;
use crate::runtime::scene_manager::action_substitution::rewrite_target;
use crate::runtime::scene_manager::SpawnId;
use crate::capabilities::player::{BoundGamepad, CharacterController, PlayerIndex, PlayerTarget};
use crate::capabilities::targeting::is_primary_player;

// ─── Resources ────────────────────────────────────────────────────────────────

/// Tracks remaining cooldown (secs) and total cooldown per slot key.
/// Entries are removed when remaining reaches 0.
/// A `BTreeMap` (sorted by slot key), not a `HashMap`: std hashers are randomly seeded, and this
/// map is iterated (`cooldown_tick_system`'s `retain`), so a hash map would give a different
/// iteration order on every run / platform (determinism: `planning/investigations/
/// hashmap_iteration_order_audit.md`).
#[derive(Resource, Default)]
pub struct CooldownMap(pub BTreeMap<String, (f32, f32)>); // (remaining, total)

/// The entity (spawn ID) currently targeted by the player.
/// Populated by the targeting system when it ships. Defaults to `None`.
/// `{target}` substitution in action bar `do_actions` resolves against this value.
#[derive(Resource, Default)]
pub struct CurrentTarget(pub Option<String>);

/// Pending slot actions held between `action_bar_input_system` and `flush_pending_intent_system`.
/// Keys are slot keys (e.g., `"1"`, `"i"`).
/// Values are (pre-target-rewritten actions, cooldown_secs).
/// Cooldown is committed by `flush_pending_intent_system` only on the commit path,
/// so a suppressed intent never starts the cooldown timer.
/// Cleared each frame by `flush_pending_intent_system`.
///
/// A `BTreeMap`, not a `HashMap`: `flush_pending_intent_system` drains it into `ActionQueue`, so
/// the iteration order IS the push order of every simultaneously-fired slot's actions. Sorted by
/// slot key it is identical on every run and platform (a hash map gave a random order: with a heal
/// `+50` and a sacrifice `-60` fired in one frame the clamped result depended on the hasher seed).
#[derive(Resource, Default)]
pub struct PendingIntentActions(pub BTreeMap<String, (Vec<Action>, Option<f32>)>);

/// Slot keys whose `intent.slot.*` event was matched by a rule this frame.
/// Written by the interpreter systems; read by `flush_pending_intent_system` to suppress
/// the slot's built-in do_actions when a designer rule took over.
/// Cleared each frame by `flush_pending_intent_system`.
#[derive(Resource, Default)]
pub struct HandledIntentSlots(pub HashSet<String>); // det: lookup-only

// ─── Components ───────────────────────────────────────────────────────────────

/// Attached to each slot button entity by the scene loader.
#[derive(Component, Clone)]
pub struct ActionSlotUi {
    pub slot_key: String,
    /// `InputMap::parse_key(&slot_key)`, resolved once at scene load. `None` if `slot_key` isn't
    /// a recognised key name (the scene loader already `warn!`s about this at spawn time) — such
    /// a slot never fires from the keyboard (there's no `KeyCode` to check `just_pressed` against);
    /// clicking the slot still fires it.
    pub resolved_key: Option<KeyCode>,
    /// `InputMap::parse_gamepad_button(&gamepad_key)`, resolved once at scene load. `None` when
    /// `gamepad_key` was omitted (ordinary keyboard-only slot) or unparseable (the scene loader
    /// already `warn!`s about this at spawn time) — such a slot never fires from gamepad.
    pub resolved_gamepad_button: Option<GamepadButton>,
    /// `InputMap::parse_mouse_button(&mouse_button)`, resolved once at scene load. `None` when
    /// `mouse_button` was omitted or unparseable (the scene loader `warn!`s about the latter). Fires
    /// only while the cursor is captured (`CursorLockState.locked`).
    pub resolved_mouse_button: Option<MouseButton>,
    pub do_actions: Vec<Action>,
    pub cooldown_secs: Option<f32>,
    pub cost: Option<SlotCost>,
    /// Copied from `ActionBarDef.owner_player`. `None` means the default single-shared-bar
    /// behavior (resolves against the primary player); `Some(n)` scopes this slot to whichever
    /// player entity carries `PlayerIndex(n)`.
    pub owner_player: Option<u32>,
}

/// The border colour of an action bar slot at rest (the loader spawns every slot with it; `bar_assign_sync_system` puts it back after the
/// assign-mode highlight). One constant, so the two can never drift.
pub const ACTION_SLOT_BORDER: Color = Color::srgba(0.45, 0.45, 0.55, 0.8);

/// Marks an item slot (item system S5): the `items.ron` key it shows and uses. A separate component, not a field of `ActionSlotUi`, so
/// player-assignable slots (S6) can insert and remove it at runtime. **It is the one source of what the slot does:** `action_bar_input_system`
/// fires `[UseItem(item)]` from it (the slot's `ActionSlotUi.do_actions` is empty for an item slot). Removing it clears the label and hides the
/// icon (`action_bar_item_slot_system` watches `RemovedComponents`); the icon and count children must already exist on the slot (S6 spawns them
/// for an assignable slot, the loader does for an authored one).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct ActionSlotItem(pub String);

/// The icon child of an item slot that uses the item's own icon (a slot with its own `icon:` keeps the static icon child instead).
/// `tint_override` is the slot's own `icon_color`, which wins over the item's.
#[derive(Component)]
pub struct SlotIcon {
    pub tint_override: Option<Color>,
}

/// The bag-count label child of an item slot. `count` is the total of the item over every stack as of the last refresh (the dim
/// reads it); `shown` is what the text currently says, so the text is only rewritten when the number changes.
#[derive(Component, Default)]
pub struct SlotItemCount {
    pub count: u32,
    /// How many one use takes (the item's `use.consume`, at least 1): the slot is dimmed, greyed and refuses while `count` is below it.
    pub needed: u32,
    pub shown: Option<u32>,
}

/// How many of the item one press of its slot needs in the bag: the item's `use.consume` (at least 1; an unknown item or one with no
/// `use:` needs 1).
pub fn item_needed(catalog: Option<&crate::schema::items::ItemCatalog>, item_key: &str) -> u32 {
    catalog
        .and_then(|c| c.items.get(item_key))
        .and_then(|d| d.use_def.as_ref())
        .map_or(1, |u| u.consume.max(1))
}

/// The label text for a bag count: `99+` above 99, so it never outgrows a small slot.
pub fn count_label(count: u32) -> String {
    if count > 99 { "99+".to_string() } else { count.to_string() }
}

/// Overlay child that fills from the top to show remaining cooldown.
/// Height is animated by `action_bar_visual_system`.
#[derive(Component)]
pub struct CooldownOverlay {
    pub slot_key: String,
}

// ─── Plugin ───────────────────────────────────────────────────────────────────

pub struct ActionBarPlugin;

fn any_action_slots(slots: Query<&ActionSlotUi>) -> bool {
    !slots.is_empty()
}

impl Plugin for ActionBarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CooldownMap>()
            .init_resource::<crate::capabilities::bar_assign::ActionBarAssignments>()
            .init_resource::<CurrentTarget>()
            .init_resource::<PendingIntentActions>()
            .init_resource::<HandledIntentSlots>()
            .add_systems(
                Update,
                (
                    cooldown_tick_system.run_if(|c: Res<CooldownMap>| !c.0.is_empty()),
                    action_bar_input_system.run_if(any_action_slots),
                    action_bar_visual_system.run_if(any_action_slots),
                )
                    .chain()
                    // After the bar-slot click owner (item system S6): a click consumed by assign mode must look consumed here.
                    .after(crate::capabilities::bar_assign::bar_assign_click_system)
                    // GameStreamSet::ActionBar (inside EmitSet::Game, after Targeting): this is what orders the action bar
                    // before the interpreters, and the targeting chain before it, explicitly (D3; it used to hold only
                    // transitively through this one edge).
                    .in_set(crate::runtime::schedule::GameStreamSet::ActionBar),
            )
            // Item slots (S5): icon and bag count are refreshed after the executor, like the bag itself, so the label never lags a use.
            // Assignable slots (S6): reconcile what each holds (before the refresh that draws it), and derive the assign mode's cancels.
            .add_systems(
                Update,
                (
                    crate::capabilities::bar_assign::apply_bar_assignments_system.before(action_bar_item_slot_system),
                    // After the bag menu's sync and before the refresh that reads what it changes, so the result is not run-order dependent.
                    crate::capabilities::bar_assign::bar_assign_sync_system
                        .after(crate::capabilities::slot_menu::slot_menu_sync_system)
                        .before(action_bar_item_slot_system),
                )
                    .in_set(crate::runtime::schedule::GameplaySet::PostExecute),
            )
            .add_systems(
                Update,
                action_bar_item_slot_system
                    .in_set(crate::runtime::schedule::GameplaySet::PostExecute)
                    // Ordered after the bag menu's sync (both write `LoadedInventoryUi`-driven UI in this set), so what it sees is not run-order dependent.
                    .after(crate::capabilities::slot_menu::slot_menu_sync_system),
            );
    }
}

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Drains all `CooldownMap` entries by the elapsed frame time; removes expired ones.
/// Skips entirely when the map is empty to avoid a pointless `ResMut` DerefMut every frame.
pub fn cooldown_tick_system(time: Res<Time>, mut cooldowns: ResMut<CooldownMap>) {
    if cooldowns.0.is_empty() { return; }
    let dt = time.delta_secs();
    cooldowns.0.retain(|_, (remaining, _total)| {
        *remaining -= dt;
        *remaining > 0.0
    });
}

/// Listens for every slot's resolved key being pressed this frame, and for each fired slot
/// either:
/// - emits `intent.slot.{key}:{entity}` + stores pending actions for the interpreter pass, or
/// - emits an `action_bar.*` event describing why the slot didn't fire.
///
/// Pending actions are flushed to `ActionQueue` by `flush_pending_intent_system` after all
/// interpreter systems run. If a designer rule matches the intent event, the slot's built-in
/// `do_actions` are suppressed; otherwise they fire unchanged.
///
/// Loops over **every** slot whose resolved key is `just_pressed`, not just the first match —
/// per-player action bars (`ActionBarDef.owner_player`, see
/// `planning/features/per_player_split_screen_targeting.md` Phase 2) mean 2+ independent bars can
/// have both players press their own key in the same frame; the earlier `find`+`return` structure
/// (see `action_bar_custom_hotkeys.md`) would have silently dropped one of them.
///
/// A slot fires on **either** device, but the two resolve differently
/// (`planning/features/gamepad_action_bar_slots.md`). Keyboard is genuinely shared hardware: any
/// slot's `key` fires from the one global `ButtonInput<KeyCode>` regardless of who's "supposed" to
/// press it — pre-existing, unchanged behavior. A gamepad is not shared the same way, so a slot's
/// `gamepad_key` only fires from its **owning player's own** controller (`owner_player` ->
/// that player's `BoundGamepad`) — never any connected pad. The fast-path skip below
/// (no keyboard press AND no `gamepad_key` bound) preserves today's exact perf profile and
/// on-unmatched-owner cooldown-event behavior for every **keyboard-only** slot (no gamepad_key at
/// all — the common case today); a slot that does declare `gamepad_key` resolves its owning
/// player every frame regardless of whether anything was actually pressed, same as any other
/// per-frame per-slot player lookup in this system.
///
/// A slot can also be **clicked**: each slot is a `Button`, so Bevy tracks an `Interaction` on it,
/// and the press edge (`Interaction` changed to `Pressed` since this system last ran — detected with
/// `Ref::is_changed`, so it fires once per press, never while held and never on release) counts as
/// a third fire source alongside keyboard and gamepad
/// (`planning/features/done/action_bar_mouse_click.md`). The mouse is shared hardware like the keyboard,
/// so a click acts for the slot's own `owner_player` regardless of which split-screen viewport the
/// cursor is in. `Interaction` is `Option`al so a bare `ActionSlotUi` (no `Button`, as in tests)
/// still matches the query. Clicks — not key presses — are ignored while the inspector is enabled,
/// since egui and Bevy UI both receive every click with no arbitration (same gate as
/// `button_system`). Click, key and button collapse into
/// one "fired" decision per slot, so simultaneous sources activate it exactly once.
///
/// Each fired slot resolves its **owning player** — `owner_player: Some(n)` matches whichever
/// player entity carries `PlayerIndex(n)`; `None` (or `Some(0)`) matches the primary player
/// (`PlayerIndex(0)` or no `PlayerIndex` at all, same definition `is_primary_player` uses
/// elsewhere). That player's own `PlayerTarget` — not the global `CurrentTarget` resource — drives
/// the `{target}` rewrite, the no-target gate, and the `intent.slot.*:{player_id}` event. For the
/// primary player this is a no-op in practice, since `PlayerTarget` is already kept in lockstep
/// with `CurrentTarget` for the primary player (Phase 1). A slot whose `owner_player` doesn't
/// match any player entity present in the scene never fires (nothing to resolve a target or an
/// acting player id against).
pub fn action_bar_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor_lock: Res<crate::capabilities::cursor_capture::CursorLockState>,
    slots: Query<(&ActionSlotUi, Option<Ref<Interaction>>, Option<&ActionSlotItem>, Option<&crate::capabilities::bar_assign::AssignableSlot>)>,
    inventory_ui: Option<Res<crate::capabilities::inventory::LoadedInventoryUi>>,
    mut game_events: MessageWriter<GameEvent>,
    cooldowns: Res<CooldownMap>,
    player_inv: Option<Res<crate::capabilities::inventory::PlayerInventory>>,
    item_catalog: Option<Res<crate::capabilities::inventory::LoadedItemCatalog>>,
    loaded_stats: Option<Res<LoadedStats>>,
    mut pending: ResMut<PendingIntentActions>,
    players: Query<(&SpawnId, &PlayerTarget, Option<&PlayerIndex>, Option<&StatMap>, &CharacterController, Option<&BoundGamepad>)>,
    gamepad_query: Query<&Gamepad>,
    sim: Res<crate::runtime::sim_clock::SimClock>,
    #[cfg(feature = "inspector")]
    inspector_enabled: Option<Res<crate::inspector::InspectorEnabled>>,
) {
    // Real pause: gated in the body, not with `run_if`, so this system keeps running and its `Ref<Interaction>`
    // change baseline keeps advancing: a slot still `Pressed` on the first frame after resume must not fire.
    if !sim.is_running() { return; }
    #[cfg(feature = "inspector")]
    let clicks_enabled = !inspector_enabled.is_some_and(|e| e.0);
    #[cfg(not(feature = "inspector"))]
    let clicks_enabled = true;

    let mut events: Vec<(&str, GameEvent)> = Vec::new();

    // Assign mode (item system S6) consumes a bar-slot click: `bar_assign_click_system` ran just before (ordered), so a click in the mode
    // neither uses a slot nor reaches the cooldown check. Key and gamepad presses still fire.
    let assign_active = inventory_ui.as_ref().is_some_and(|u| u.assign_mode.is_some());

    for (slot, interaction, item, assignable) in slots.iter() {
        // Keyboard and mouse are shared hardware: both are "device-independent" fires, gated
        // before the owning player is resolved (the same ordering the keyboard-only path always
        // had, preserved byte-for-byte). Distinct from the gamepad, which needs its owning
        // player's own controller and is therefore resolved after the player lookup.
        let key_pressed = slot.resolved_key.is_some_and(|kc| keys.just_pressed(kc));
        // A slot with a `mouse_button` is fired by that button while the mouse is captured; a click on its own
        // on-screen box must not fire it as well (the click that captures the mouse could land on the box, and
        // with a free cursor a click on the HUD would shoot), so such a slot ignores UI clicks entirely.
        let click_fired = clicks_enabled
            && !assign_active
            && slot.resolved_mouse_button.is_none()
            && interaction.is_some_and(|i| i.is_changed() && *i == Interaction::Pressed);
        // A mouse-button slot fires only while the cursor is captured: with a free cursor a click is aimed at
        // the page or an in-game panel, and the click that captures the cursor must not also shoot (the
        // capture system runs after this one in the frame, so that click still sees `locked == false`).
        let mouse_pressed = cursor_lock.locked
            && slot.resolved_mouse_button.is_some_and(|b| mouse.just_pressed(b));
        let direct_fired = key_pressed || click_fired || mouse_pressed;
        // Fast path: unchanged perf profile for the common case (no gamepad binding, not pressed).
        if !direct_fired && slot.resolved_gamepad_button.is_none() { continue; }

        let key_str = slot.slot_key.as_str();

        // ── Cooldown check (keyboard / click) ────────────────────────────────────────────
        // Gated before player resolution, exactly as before this feature — preserves the
        // keyboard-only on-unmatched-owner cooldown-event behavior byte-for-byte. A gamepad press
        // can't be checked yet: `gamepad_fired` needs the owning player's own `gamepad_index`,
        // resolved below.
        if direct_fired && cooldowns.0.contains_key(key_str) {
            events.push((key_str, GameEvent::Trigger(
                format!("action_bar.on_cooldown:{}", key_str),
            )));
            continue;
        }

        // ── Resolve the acting player for this slot ─────────────────────────────
        let Some((spawn_id, player_target, _, player_stats, _controller, bound)) = players.iter()
            .find(|(_, _, idx, _, _, _)| owns_slot(slot.owner_player, *idx))
        else { continue };

        let gamepad_fired = slot.resolved_gamepad_button.is_some_and(|btn| {
            bound.and_then(|b| b.0).and_then(|e| gamepad_query.get(e).ok())
                .is_some_and(|gp| gp.just_pressed(btn))
        });
        if !direct_fired && !gamepad_fired { continue; }

        // ── Cooldown check (gamepad-only fire) ───────────────────────────────────
        // Mirrors the keyboard check above for the one case it couldn't cover: a gamepad press
        // with no keyboard press or click this frame. `direct_fired` presses were already handled (and
        // returned) above, so this can't double-emit.
        if !direct_fired && cooldowns.0.contains_key(key_str) {
            events.push((key_str, GameEvent::Trigger(
                format!("action_bar.on_cooldown:{}", key_str),
            )));
            continue;
        }

        // ── An empty assignable slot (item system S6) does nothing: no cooldown, no `intent.slot.*`, no `activated` ───
        // Without this an empty slot would still build an empty intent and the flush would start its cooldown.
        // The loader bakes no `do_actions` into an assignable slot, so "no item" is all the gate needs.
        if assignable.is_some() && item.is_none() {
            events.push((key_str, GameEvent::Trigger(
                format!("action_bar.empty_slot:{}", key_str),
            )));
            continue;
        }

        // ── Item slot: the bag must hold the item (item system S5) ───────────────
        // After the owning player is resolved and the cooldown checks, before the cost: cooldown, then no-item, then cost. A refusal
        // pushes nothing, starts no cooldown, spends no cost and emits neither `action_bar.pressed` nor `intent.slot.*` (so a rule on the
        // intent never sees a press on an empty slot), like `insufficient_resource`. Reads the bag as the last executor left it.
        if let Some(item) = item {
            let held = player_inv.as_ref().map_or(0, |p| crate::capabilities::item_txn::count_item(&p.slots, &item.0));
            // One use takes `use.consume` (at least 1): fewer held than that would only fail inside `UseItem` after the cost was spent.
            let needed = item_needed(item_catalog.as_deref().and_then(|c| c.0.as_ref()), &item.0);
            if held < needed {
                events.push((key_str, GameEvent::Trigger(
                    format!("action_bar.no_item:{}", key_str),
                )));
                // Also the form that names the item: an assignable slot holds whatever the player put there.
                events.push((key_str, GameEvent::Trigger(
                    format!("action_bar.no_item:{}:{}", key_str, item.0),
                )));
                continue;
            }
        }

        // ── Cost check ────────────────────────────────────────────────────────
        // Resolved once (own StatMap vs. global LoadedStats) and reused below for the deduct —
        // must not be independently re-resolved at each site, or the two could disagree about
        // which pool to hit (system-architect finding, per_player_stat_pools.md).
        // The cost gate reads `.current` (not `effective`) from the previous frame's stat pass: `GameStreamSet::Stats` runs
        // after the action bar in the same frame, so a gate decision sees last frame's regen (one frame, ~16 ms, deterministic).
        let cost_resolution = slot.cost.as_ref().map(|cost| {
            resolve_cost_source(&cost.stat, player_stats, loaded_stats.as_deref())
        });
        if let (Some(cost), Some((current, _))) = (&slot.cost, &cost_resolution) {
            if *current < cost.amount {
                events.push((key_str, GameEvent::Trigger(
                    format!("action_bar.insufficient_resource:{}", key_str),
                )));
                continue;
            }
        }

        // ── {target} check ────────────────────────────────────────────────────
        let needs_target = slot.do_actions.iter().any(action_needs_target);
        if needs_target && player_target.0.is_none() {
            events.push((key_str, GameEvent::Trigger(
                format!("action_bar.no_target:{}", key_str),
            )));
            continue;
        }

        // ── Fire ─────────────────────────────────────────────────────────────
        let target_id = player_target.0.as_deref().unwrap_or("");

        // Emit the intent event. The interpreter checks for a matching rule this frame;
        // if one matches, flush_pending_intent_system suppresses the slot's built-in do_actions.
        events.push((key_str, GameEvent::Trigger(
            format!("intent.slot.{}:{}", key_str, spawn_id.0),
        )));

        // Store pending actions (target-rewritten) + cooldown. Flushed to ActionQueue by
        // flush_pending_intent_system unless a rule handled the intent.
        // Cooldown and `action_bar.activated` are also deferred to flush so that a
        // suppressed intent never starts the cooldown or fires the result event.
        // An item slot fires `UseItem(item)` from its `ActionSlotItem` (the one source of truth: S6 inserts and removes it at runtime).
        let mut actions: Vec<Action> = match item {
            Some(item) => vec![Action::UseItem(item.0.clone())],
            None => slot.do_actions.iter().map(|a| rewrite_target(a.clone(), target_id)).collect(),
        };
        if let (Some(cost), Some((_, use_player_pool))) = (&slot.cost, &cost_resolution) {
            let key = if *use_player_pool {
                format!("{}.{}", spawn_id.0, cost.stat)
            } else {
                cost.stat.clone()
            };
            actions.push(Action::ModifyStat { key, delta: -cost.amount });
        }
        pending.0.insert(key_str.to_string(), (actions, slot.cooldown_secs));

        // action_bar.pressed fires immediately (before interpreter) — notification that the key
        // was pressed and passed all gate checks. Use for telemetry or UI feedback that should
        // fire regardless of whether a rule later cancels the intent.
        events.push((key_str, GameEvent::Trigger(
            format!("action_bar.pressed:{}", key_str),
        )));
    }

    // Emit in slot-key order, not query order: `slots.iter()` follows entity/archetype order, which
    // is not stable across machines or runs, and the interpreters read these events in write order.
    // Buffered (a `Vec` only allocates once an event is pushed, so an idle frame costs nothing) and
    // stable-sorted so one slot's own events keep their relative order. Two bars sharing a slot
    // key (already a validate error) tie and keep query order.
    events.sort_by(|a, b| a.0.cmp(b.0));
    for (_, event) in events {
        game_events.write(event);
    }
}

/// Whether the player entity carrying `idx` (its `PlayerIndex`, if any) owns a slot whose bar
/// declared `owner_player`. `None`/`Some(0)` both mean "the primary player" — matches
/// `is_primary_player`'s existing "`PlayerIndex(0)` or no `PlayerIndex` at all" definition.
fn owns_slot(owner_player: Option<u32>, idx: Option<&PlayerIndex>) -> bool {
    match owner_player {
        None | Some(0) => is_primary_player(idx),
        Some(n) => idx.is_some_and(|i| i.0 == n),
    }
}

/// Resolves a `SlotCost.stat` against the acting player's own `StatMap` first, falling back to
/// the global `LoadedStats` resource exactly as before per-player stat pools existed. Returns the
/// current value plus whether the player's own pool was the source — deliberately does **not**
/// build the dot-routed deduct key here (that requires a `format!` allocation): this is called
/// every frame from `action_bar_visual_system` for every cost-gated slot, so building a string
/// that call site immediately discards would allocate on a per-frame hot path for no reason
/// (wasm-perf-reviewer finding). The one call site that actually needs the key
/// (`action_bar_input_system`'s deduct push) builds it itself, already gated behind
/// `just_pressed` — see that call site.
///
/// Called once per firing slot and the result reused for both the cost gate and the deduct
/// action's key (see the two call sites in `action_bar_input_system` and
/// `action_bar_visual_system`) — computing this independently at each site risks the two
/// disagreeing about which pool a slot's cost resolves against. See
/// `planning/features/per_player_stat_pools.md`.
fn resolve_cost_source(
    stat: &str,
    player_stats: Option<&StatMap>,
    loaded_stats: Option<&LoadedStats>,
) -> (f32, bool) {
    // One rule for the bar, shops, loot and the gold footer: `capabilities::item_txn::resolve_stat_source`.
    crate::capabilities::item_txn::resolve_stat_source(stat, player_stats, loaded_stats)
}

/// Drains `PendingIntentActions` after all interpreter systems have run.
/// For each pending slot:
/// - If a rule handled its intent (`HandledIntentSlots` contains the key): suppressed — no
///   actions, no cooldown, no `action_bar.activated` event.
/// - Otherwise: pushes actions to `ActionQueue`, commits the cooldown, and emits
///   `action_bar.activated:{key}` so rules can react to the committed ability.
pub fn flush_pending_intent_system(
    mut pending: ResMut<PendingIntentActions>,
    mut handled: ResMut<HandledIntentSlots>,
    mut action_queue: ResMut<ActionQueue>,
    mut cooldowns: ResMut<CooldownMap>,
    mut game_events: MessageWriter<GameEvent>,
) {
    // `take` (not `drain`, which `BTreeMap` lacks): iterate in sorted slot-key order, leave empty.
    for (slot_key, (actions, cooldown)) in std::mem::take(&mut pending.0) {
        if !handled.0.contains(&slot_key) {
            for action in actions {
                action_queue.push(action);
            }
            if let Some(cd) = cooldown {
                cooldowns.0.insert(slot_key.clone(), (cd, cd));
            }
            game_events.write(GameEvent::Trigger(format!("action_bar.activated:{}", slot_key)));
        }
    }
    handled.0.clear();
}

/// Updates cooldown overlay alpha each frame.
/// Uses alpha-fade only — no `Node` height writes, so Bevy's UI layout is never invalidated.
/// Alpha encodes both cooldown progress (fades out as cooldown depletes) and blocked state
/// (constant dim when cost stat is insufficient).
pub fn action_bar_visual_system(
    cooldowns: Res<CooldownMap>,
    loaded_stats: Option<Res<LoadedStats>>,
    slots: Query<(&ActionSlotUi, &Children, Option<&ActionSlotItem>)>,
    players: Query<(Option<&PlayerIndex>, Option<&StatMap>), With<CharacterController>>,
    mut overlays: Query<(&CooldownOverlay, &mut BackgroundColor)>,
    counts: Query<&SlotItemCount>,
) {
    for (slot, children, item) in &slots {
        let cd_frac = cooldowns.0.get(&slot.slot_key)
            .map(|(remaining, total)| (remaining / total).clamp(0.0, 1.0))
            .unwrap_or(0.0);

        // Same per-player-first, global-fallback resolution as action_bar_input_system's cost
        // check, so the dim overlay never disagrees with whether the slot will actually fire.
        // Only the current value is needed here, never the dot-routed deduct key — resolve_cost_
        // source is written so this never allocates a string on this per-frame path.
        let cost_ok = slot.cost.as_ref().map(|c| {
            let player_stats = players.iter()
                .find(|(idx, _)| owns_slot(slot.owner_player, *idx))
                .and_then(|(_, ps)| ps);
            resolve_cost_source(&c.stat, player_stats, loaded_stats.as_deref()).0 >= c.amount
        }).unwrap_or(true);

        // An item slot with none in the bag is dimmed like a slot blocked by cost (the count is what the last refresh stored).
        // Only item slots look at the count (a plain slot pays nothing for it).
        let item_ok = item.is_none() || children.iter().find_map(|c| counts.get(c).ok()).map_or(true, |c| c.count >= c.needed.max(1));

        // On cooldown: alpha tracks remaining fraction (bright when just used, fades as it clears).
        // Blocked by cost, or an item slot with none left: constant dim at 0.45.
        // Ready: transparent.
        let target_alpha: f32 = if cd_frac > 0.0 {
            0.25 + cd_frac * 0.45
        } else if !cost_ok || !item_ok {
            0.45
        } else {
            0.0
        };

        for child in children.iter() {
            if let Ok((overlay, mut bg)) = overlays.get_mut(child) {
                if overlay.slot_key != slot.slot_key { continue; }
                if (bg.0.alpha() - target_alpha).abs() > 0.01 {
                    bg.0 = bg.0.with_alpha(target_alpha);
                }
            }
        }
    }
}

/// Refreshes every item slot's icon and bag count (item system S5). `GameplaySet::PostExecute`, so it sees the bag after this frame's
/// actions. Does nothing on an idle frame: it runs only when the bag, the item catalog or the inventory UI (the icon atlases) changed,
/// or a slot's item was just added or changed (S6 will insert and remove `ActionSlotItem` at runtime). Writes are guarded.
#[allow(clippy::too_many_arguments)]
pub fn action_bar_item_slot_system(
    player_inv: Option<Res<crate::capabilities::inventory::PlayerInventory>>,
    catalog: Res<crate::capabilities::inventory::LoadedItemCatalog>,
    inv_ui: Res<crate::capabilities::inventory::LoadedInventoryUi>,
    // `Added<SlotItemCount>` would conflict with the `&mut SlotItemCount` query; a slot and its children spawn together, so the slot's
    // `Changed<ActionSlotItem>` (which includes its first insert) covers a new label too.
    fresh: Query<(), Or<(Changed<ActionSlotItem>, Added<SlotIcon>)>>,
    mut removed: RemovedComponents<ActionSlotItem>,
    items: Query<&ActionSlotItem>,
    assignables: Query<(), With<crate::capabilities::bar_assign::AssignableSlot>>,
    mut counts: Query<(&ChildOf, &mut SlotItemCount, &mut Text, &mut TextColor)>,
    mut icons: Query<(&ChildOf, &SlotIcon, &mut ImageNode, &mut Visibility)>,
    mut warned: Local<std::collections::BTreeSet<String>>,
) {
    let bag_changed = player_inv.as_ref().is_some_and(|p| p.is_changed());
    // A removed `ActionSlotItem` (S6 clearing a slot) must be noticed too: its label and icon are cleared below.
    let removed_any = removed.read().count() > 0;
    if !(bag_changed || removed_any || catalog.is_changed() || inv_ui.is_changed() || !fresh.is_empty()) {
        return;
    }
    for (parent, mut count, mut text, mut color) in counts.iter_mut() {
        let Ok(item) = items.get(parent.parent()) else {
            // No item: an empty assignable slot shows a faint `+`, any other slot an empty label (the dim only reads item slots).
            let (mark, mark_color) = if assignables.get(parent.parent()).is_ok() {
                ("+", Color::srgba(0.6, 0.6, 0.6, 0.6))
            } else {
                ("", Color::srgba(1.0, 1.0, 1.0, 0.95))
            };
            if text.0 != mark { text.0 = mark.to_string(); }
            if color.0 != mark_color { color.0 = mark_color; }
            if count.shown.is_some() || count.count != 0 || count.needed != 0 {
                count.shown = None;
                count.count = 0;
                count.needed = 0;
            }
            continue;
        };
        let held = player_inv.as_ref().map_or(0, |p| crate::capabilities::item_txn::count_item(&p.slots, &item.0));
        let needed = item_needed(catalog.0.as_ref(), &item.0);
        if count.needed != needed { count.needed = needed; }
        if count.count != held { count.count = held; }
        if count.shown != Some(held) {
            text.0 = count_label(held);
            count.shown = Some(held);
        }
        let want = if held < needed { Color::srgba(0.55, 0.55, 0.55, 0.9) } else { Color::srgba(1.0, 1.0, 1.0, 0.95) };
        if color.0 != want { color.0 = want; }
    }
    for (parent, marker, mut image, mut vis) in icons.iter_mut() {
        let Ok(item) = items.get(parent.parent()) else {
            if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
            continue;
        };
        let icon = crate::capabilities::inventory::resolve_item_icon(catalog.0.as_ref(), &inv_ui, &item.0);
        let Some((tex, layout)) = icon.atlas else {
            if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
            // Once per item: no picture can be found (a scene with no InventoryPanel, or an unknown item).
            if !warned.contains(&item.0) {
                warned.insert(item.0.clone());
                warn!(
                    "ActionBar item slot '{}': no icon found (needs an InventoryPanel in the scene to read the item's sheet from, or set `icon:` on the slot)",
                    item.0
                );
            }
            continue;
        };
        warned.remove(&item.0); // it resolves now: a later failure warns again
        if image.image != *tex { image.image = tex.clone(); }
        if let Some(ta) = image.texture_atlas.as_mut() {
            if ta.layout != *layout { ta.layout = layout.clone(); }
            if ta.index != icon.index { ta.index = icon.index; }
        }
        let tint = marker.tint_override.unwrap_or(icon.tint);
        if image.color != tint { image.color = tint; }
        if *vis != Visibility::Inherited { *vis = Visibility::Inherited; }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn action_needs_target(action: &Action) -> bool {
    match action {
        Action::ModifyStat { key, .. } => key.contains("{target}"),
        Action::SetStat { key, .. } => key.contains("{target}"),
        Action::SpawnEffect { entity, .. } => entity.as_deref() == Some("{target}"),
        Action::ShowDamagePopup { entity, .. } => entity.contains("{target}"),
        Action::ShowFloatingText { entity, .. } => entity.contains("{target}"),
        Action::SetEntityVisible { entity, .. } => entity.contains("{target}"),
        Action::Despawn(s) => s.contains("{target}"),
        Action::SetDespawnTimer { entity, .. } => entity.contains("{target}"),
        Action::EmitEvent(s) => s.contains("{target}"),
        Action::PlayAnimationOn { target, .. } => target.contains("{target}"),
        Action::Spawn { at_entity, .. } => at_entity.as_deref().is_some_and(|e| e.contains("{target}")),
        // Fieldless: nothing to substitute (explicit, so a reader knows pause was considered).
        Action::PauseGame | Action::ResumeGame | Action::TogglePause => false,
        _ => false,
    }
}

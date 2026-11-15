# Feature: Using items (item system S3)

_Status: Ready_
_Planned at: `60e28b0` (2026-11-14)_
_Plan-reviewed 2026-11-15: system-architect and ux-gamedesigner-reviewer both returned "needs more design work"; Frank's answers to the
two open decisions and every point from both reviews are folded in below._

Slice S3 of `planning/features/item_system_review.md`. Builds on S2a (the item transaction primitive,
`planning/features/item_transaction_primitive.md`, done) and S1 (panel state, done). S4 (sell, destroy, the context menu), S5/S6 (action
bar item slots) and the item use *conditions* item (health/mana checks, equip rules, merging items) are NOT part of this slice.

## What
A designer can make an item usable: `items.ron` gets an optional `use:` block (a list of actions and how many to consume), and the
player can click an item in the inventory to use it. Today the demo's health and mana potions can be bought and looted but not used.
Using an item takes it from the bag and, only if that worked, runs its actions (`ModifyStat`, `ShowFloatingText`, `SpawnEffect`, ...).
The same slice makes the engine's two panel verbs "default-backed": a shop Buy button or an inventory click runs the built-in action
unless the project's `state_machine.ron` has a rule for that exact event, so a project no longer needs one hand-written rule per
item (existing rules keep working and win).

## Why
Potions that cannot be used make the inventory a list. The action bar can already fire `[RemoveItem(..), ModifyStat(..)]`, but that
heals even when `RemoveItem` removed nothing (the atomicity bug S2a fixed for buying). S4, S5 and S6 all build on one "use this item"
action with a defined failure event. The per-item rule problem (`state_machine.ron:176-179`, one rule per stocked item just to make Buy
work) would otherwise repeat for every new verb.

## Decisions (Frank, 2026-11-15, "go with your recommendations")
1. **A rule "wins" when it is bound anywhere.** If `state_machine.ron` has a binding for the exact event
   `ui.button_pressed:buy_item:{key}` or `ui.button_pressed:use_item:{key}` in `global_on` or any state's `on:` list, the designer owns
   that event and the engine never runs the default for it, in any state. This keeps today's behaviour for existing projects (the
   demo's Buy rules live in `playing`, so a Buy click in another state still does nothing). Only bindings count: a transition's `on`
   does not (a tutorial transition must not silently cancel a drink), and neither do entity `.behavior.ron` bindings (documented).
   Computed from the loaded state machine when it loads; no change to the interpreters. To react to a use without replacing it, listen to
   `item.used:{item}`.
2. **Click and right-click use directly in S3.** Left click, tap and right click all Use. The Use/Sell/Destroy menu (earlier decision 9)
   arrives in S4, where it has 2+ verbs and left click then opens it; right-click = Use stays as the shortcut. Docs say so.

Adopted from the reviews without a separate question: `use:` takes `do_actions` (not `actions`); effects run right after the use
(front-inserted), not after the rest of the batch; `UseItem` inside an item's own `use:` is forbidden; `consume: 0` means "must hold one,
none is taken"; the built-in click skips items with no `use:`; `{self}` is the primary player, `{target}` is not available in `use:`;
`UseItem` emits `item.used` only (not `inventory.removed`); no built-in message on use.

## Approach
1. **Schema.** `ItemDef.use_def: Option<ItemUseDef>` (RON field `use:`, via `#[serde(rename = "use")]`; `use` is a Rust keyword),
   `ItemUseDef { do_actions: Vec<Action>, consume: u32 /* default 1 */ }`. Additive (`#[serde(default)]`), no `schema_version` bump.
   `items.ron` becomes an Action-bearing file for the first time. Its loader already reports a parse failure (`project_loader.rs:225-235`,
   `error!` and `LoadedItemCatalog(None)`), but the blast radius is the whole file (icons vanish, everything stacks to 99, coins become
   bag items): add a test that a bad action in `items.ron` logs and the project still loads, and say it in the docs.
2. **`Action::UseItem(item_key)`** (tuple variant, like `BuyItem`; "catalog key, no substitution" group in `rewrite_self`/`rewrite_target`).
   In the executor, through the S2a primitive:
   - Fails (changes nothing, runs nothing, emits `item.use_failed` and `item.use_failed:{item}`) when the item is unknown or has no
     `use:`, the bag holds fewer than `max(consume,1)`, or an action contains `{self}` and there is no player.
   - Otherwise `plan([Take{Player, item, consume}])` (for `consume: 0` only the "holds one" check, nothing taken), writes the bag back,
     emits `item.used:{item}`, then **front-inserts** the item's `do_actions` with `{self}` rewritten to `primary_player_id`
     (`ActionQueue::push_front_all`, inserted in reverse to keep authored order; executor-only, documented as the one exception to
     "push order = execution order"; interpreters never call it). So `[UseItem(x), LoadScene(..)]` runs the effects before the load.
   - **Loop guard:** the executor drains until the queue is empty in the same frame, so a `UseItem` inside any item's `do_actions`
     could freeze the frame. A `UseItem` found in `use.do_actions` is skipped with a `warn!` at runtime and is an error in `ironhold validate`.
3. **Clickable inventory slots.** The slot nodes (`scene_loader.rs:2612-2621`, plain `Node`s today) become `Button`s with no `UiAction`
   (`button_system` ignores them and does not recolour them; the panel root keeps `FocusPolicy::Block`, so clicks do not reach the world
   targeting or the action bar behind). A new `inventory_slot_click_system`, chained inside `EmitSet::Ui` after `icon_button_click_system`
   (two unordered `UiEvent` writers fail the schedule test), emits `UiEvent::ButtonPressed("use_item:{item_key}")` for the item in the
   clicked slot, read from `PlayerInventory` at click time: left click or tap (`Ref<Interaction>` edge to `Pressed`, like the action
   bar slot click) and right click (`ButtonInput<MouseButton>` just pressed while the slot is `Hovered`; bevy_ui only sets `Pressed` for
   left/touch). Empty slots emit nothing. It reads `SimClock` (pause gate in the function body so the `Interaction` baseline keeps
   advancing, like `action_bar_input_system`; added to `schedule_order_tests`' `gated` list) and is ignored while `InspectorEnabled`.
   The event fires for any non-empty slot, including an item with no `use:` (a rule, and S4's menu, can catch it).
4. **Default-backed panel events.** `default_panel_actions_system` (`GameplaySet::Execute`, ordered explicitly **after**
   `flush_pending_intent_system` and before the executor; a new sanctioned `ActionQueue` pusher, listed in `runtime/schedule.rs`
   "order the docs promise" and `docs/30` "Same-frame event order") reads the frame's `ButtonPressed` events. For `buy_item:{key}` it pushes
   `BuyItem(key)`, for `use_item:{key}` it pushes `UseItem(key)` (skipped when the item has no `use:`), unless that exact event is in
   `BoundPanelEvents`. `BoundPanelEvents` is a lookup-only set rebuilt when `LoadedStateMachine` changes (`is_changed()` inside the
   system): every `ui.button_pressed:buy_item:*` / `use_item:*` binding in `global_on` or any state's `on:`. Only Buy and inventory-slot Use have a default;
   `close_*` and `take_all_from_container` still need rules.
5. **CLI.** Remove the hard error for a missing `buy_item:{key}` rule (`validate.rs:3057-3071`, "buying it will do nothing": now false);
   `collect_reachable_ui_triggers` (`validate.rs:3110-3166`) adds `use_item:{key}` for every item with `use:` when an `InventoryPanel` exists,
   so `--strict` does not report the override example as an orphan; no forward check for `use_item`. `collect_actions` gains an items source
   (feeds item-key/stat-key/sound checks and `discover_extra_scenes`). Validate: `UseItem(key)` item exists and has `use:`; `UseItem`
   inside `use.do_actions` is an error; `{target}` inside `use.do_actions` is an error; an empty `do_actions` warns; `use:` on a currency
   item warns. Messages and `query` say `use`, never `use_def`. `query actions` lists `UseItem`.
6. **Demo.** `health_potion`: `use: ( do_actions: [ ModifyStat(key: "player_health", delta: 40.0), ShowFloatingText(entity: "{self}", text: "+40 HP") ], consume: 1 )`
   (the free Heal spell in bar slot 5 is +25); `mana_potion`: `+30` on `player_mana` with `"+30 MP"`. The four explicit `buy_item:*` rules stay with
   a comment that they are now optional (they are the "rule wins" example). A short HUD hint: "I: bag - right-click a potion to drink" (plain ASCII, no em dash).
   Verify the right click on web does not open the browser context menu on the canvas and a right-click on a slot does not orbit the camera
   or turn the character (the demo's `orbit_button` is `"Right"`; `camera.rs:218-219` has no UI-hover check, existing behaviour: log a backlog item if it leaks).

### Events
New: `item.used:{item}` (specific form only, like `item.bought`; do not write a bare `item.used` rule), `item.use_failed` and
`item.use_failed:{item}` (these keep firing if a later version adds a reason form). A failed use never emits `item.used`.

### RON authoring examples
```ron
// items.ron
"health_potion": (
    display_name: "Health Potion", icon_index: 2, stackable: true, max_stack: 10, tags: ["consumable"],
    use: ( do_actions: [ ModifyStat(key: "player_health", delta: 40.0), ShowFloatingText(entity: "{self}", text: "+40 HP") ], consume: 1 ),
),
"war_horn": ( display_name: "War Horn", stackable: false, use: ( do_actions: [ PlaySound(key: "horn") ], consume: 0 ) ),  // reusable: must hold one, none taken

// state_machine.ron: optional overrides (a rule bound in any state replaces the built-in default for that exact event)
( event: "ui.button_pressed:use_item:health_potion", do_actions: [ UseItem("health_potion"), ShowFloatingText(entity: "player_01", text: "Ahh") ] ),
( event: "item.use_failed",                          do_actions: [ ShowFloatingText(entity: "player_01", text: "None left") ] ),
( event: "item.used:mana_potion",                    do_actions: [ EmitEvent("quest.mana_potion_used") ] ),
// switch the built-in Buy off in a state: bind the event in that state with no actions
( event: "ui.button_pressed:buy_item:old_key", do_actions: [] ),

// gamepad/keyboard users cannot click the bag: give them an action bar slot
( key: "6", gamepad_key: "DPadUp", do_actions: [ UseItem("health_potion") ], cooldown_secs: 2.0, label: "Potion" ),
```

## Tasks
- [ ] Failing tests first (red against today's code): using a potion consumes one, runs its actions once and emits `item.used`; none held, no
      `use:`, unknown item and no-player-with-`{self}` all emit `item.use_failed` + `:{item}` and change nothing; `consume: 2` needs two;
      `consume: 0` needs one and takes none; `{self}` resolves to the primary player; `[UseItem(x), CloseInventory]` runs the effects before the close;
      `UseItem` inside `use.do_actions` is skipped; a rule bound anywhere for `use_item:x` / `buy_item:x` suppresses the default in every state
      (no double use, no double buy); a state-scoped Buy rule outside its state does not buy; a project with no rule buys and uses; the
      default skips an item with no `use:`; a bad action in `items.ron` logs and the project still loads; slot left click, right click
      and a paused clock; an empty slot emits nothing
- [ ] Schema `ItemUseDef` and `Action::UseItem` (every exhaustive match: `schema/actions.rs`, both substitution fns, executor, CLI `query`/`validate`)
- [ ] `item_actions::use_item`, `ActionQueue::push_front_all`, the loop guard, `consume: 0`
- [ ] `BoundPanelEvents` + `default_panel_actions_system` (ordered after `flush_pending_intent_system`), `schedule.rs` doc, `schedule_order_tests` green
- [ ] Slot `Button`s and `inventory_slot_click_system` (left, right, tap, pause gate, inspector gate, `EmitSet::Ui` order)
- [ ] `ironhold validate` changes (remove the buy hard error, `use_item` reachable, `use.do_actions` walked, loop/`{target}` errors, warnings), `validate_cross_file` tests both directions
- [ ] Docs: `docs/20` (`ItemDef` `use` row with a `do_actions`/`consume` sub-table and the full-health note; a "Using items" paragraph: rule wins, bound in any
      state, how to switch off in a state, add vs replace, no message on use, double-click drinks two, `{self}` is player 1 and `{target}` unavailable,
      gamepad/keyboard via a bar slot, only Buy and Use have defaults, what survives, out of scope; events table; reword the buy-messages promise
      "a later release lets a rule replace them", which S3 does not deliver), `docs/30` (events, `UseItem` entry, same-frame order),
      `docs/15` validate rows, `STATUS.md`, `runtime/scene_manager/CLAUDE.md` (the front-insert exception, the default pusher), the demo
- [ ] Reviews, full core test loop, `cargo check -p ironhold_cli`, `cargo test -p ironhold_cli`, `ron_lint`/`ron_validation`

## Out of scope (stated in the docs' "Not supported yet")
Equipping, item cooldowns (note for S5: an inventory click bypasses a bar slot's `cooldown_secs`, so item cooldowns probably belong on `use:`), cast or drink
times, use-on-target or aiming, over-time effects (`ApplyModifier` is the nearest tool), combining or crafting, use conditions (backlog item), the Use/Sell/Destroy
menu (S4), hover tooltips, gamepad/keyboard bag navigation, drag to the bar. Known limitation until the use-conditions item: a health potion used at full health is consumed for nothing.

## Open questions
None blocking. (Decide during implementation: whether entity `.behavior.ron` bindings should also count as "bound"; documented as not counting.
Follow-ups to log, not part of S3: the built-in buy messages becoming replaceable by a rule; a right-click leaking to the camera if the playtest shows it.)

## Acceptance criteria
- Given a health potion in the bag, when the player clicks it or right-clicks it, then one potion is consumed, `item.used:health_potion` fires and the
  item's actions run once, before anything queued behind the use.
- Given no potion held, when `UseItem("health_potion")` runs, then nothing is consumed, no action runs and `item.use_failed` and
  `item.use_failed:health_potion` fire.
- Given a rule on `ui.button_pressed:use_item:health_potion` in any state, when the slot is clicked, then only the rule runs, in every state.
- Given a project with no `buy_item:*` rules, when the player clicks Buy, then `BuyItem` runs; given the demo's existing rules, one buy per click.
- Given a sword with no `use:`, when it is clicked, then nothing is consumed and no `item.use_failed` fires (the click event still reaches rules).
- Given `consume: 0` and one horn held, when used, then its actions run and the horn stays.
- `ironhold validate` no longer asks for a `buy_item:*` rule and rejects `UseItem` inside an item's `use:`.

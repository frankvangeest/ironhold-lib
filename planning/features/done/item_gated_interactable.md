# Feature: Item-gated interactable

_Status: Done_
_Planned at: `b1fc32c` (2026-09-14)_

## What

Lets a designer author an `interactable` entity (a door, chest, gate) that only responds normally
when the player is carrying a specific item — a key-locked door, a quest-gated trigger — without
faking it through a `GameVariable` set at item-*purchase* time (today's only workaround, which
doesn't re-lock if the item is later lost or traded away). A new optional `requires_item: String`
field on `InteractableDef` names an `items.ron` catalog key; `interactable_system` checks the
player's `PlayerInventory` for that key before firing the normal interact event, and fires a new
`entity.interact_blocked:{id}` event instead when the player doesn't have it.

## Why

Flagged independently by two review lenses in `planning/stakeholder_priority_list.md`
(game-world-designer's #2, citing a concrete blocked design — Greywatch's Seal Door needs
`old_key`). Locked doors are one of the oldest legible world-building tools there is: they
communicate a barrier spatially instead of through a dialogue wall, and make items feel like they
matter. The engine already has everything this needs except the gate check itself — `Inventory`
system, `interactable_system`, and the `entity.*` event convention are all shipped; this is a small,
additive schema surface with disproportionate narrative payoff, not a new subsystem.

## Approach

- **Schema** (`schema/catalog.rs`): add `requires_item: Option<String>` (`#[serde(default)]`) to
  `InteractableDef`. `None` (the default) is byte-for-byte today's behavior — no migration, no
  existing RON needs to change. `hint_text` stays unrendered/reserved and unaffected — a locked
  door's `hint_text` (e.g. `"Open"`) does not change to "Locked" or similar; that's a future UI-pass
  concern, out of scope here.
- **Component** (`capabilities/interactable.rs`): mirror the new field onto the `Interactable`
  component (scene loader already copies `InteractableDef` fields onto it 1:1; single construction
  site, `entity_spawner.rs`'s `attach_prefab_features`).
- **System** (`interactable_system`): needs `Res<PlayerInventory>` added as a param. For each
  in-range `Interactable` with `requires_item: Some(key)`, check whether `PlayerInventory.slots`
  contains that `item_key` (new small helper `capabilities::inventory::has_item(slots, key) -> bool`,
  since no such check exists today — `add_to_slots`/`remove_from_slots` are the only helpers, and
  neither is a pure lookup). If present: fire `entity.interacted:{id}` exactly as today (no double
  gate — possession alone unlocks it, consistent with "should re-lock if lost/traded" from the Why
  section, since traded-away means it's no longer in `PlayerInventory`). If absent: fire
  `entity.interact_blocked:{id}` instead of `entity.interacted:{id}`, but **still counts toward
  `hit_any`** (system-architect review: the press *did* land on something and was correctly
  refused — that is not the same failure as "nothing in range," so `player.attack_missed` must not
  fire for a blocked interact; a first draft of this plan had the two backwards).
- **New event**: `entity.interact_blocked:{id}` — same `entity.*` family and same exact-match-only
  semantics as `entity.interacted:{id}`/`entity.entered:{id}`/`entity.exited:{id}` (there is no
  `entity.interact_blocked:*` wildcard, matching how `entity.interacted` itself already works — a
  global rule reacting to it still needs the entity's literal spawn id, same as any other
  per-entity `entity.*` event). Deliberately carries only the entity id, not which item is missing
  or a reason code — matches `entity.interacted:{id}`'s own shape, and the missing item is already
  knowable from the prefab's own `requires_item` field if a designer's handler needs it. A designer
  wires the response in the entity's own `.behavior.ron` `on:`/`global_on:` list, or a project-wide
  `global_on:` rule (both are the same `StateMachineAsset` schema per
  `crates/ironhold_core/src/CLAUDE.md`) — e.g. `ShowFloatingText`/`PlaySound` "locked" feedback,
  exactly like any other `entity.*` event; no new `Action` needed.
- **Design-time diagnostics, two layers (system-architect review — the CLI check alone is not
  enough):**
  1. `ironhold_cli validate` reference check mirroring the existing
     `inventory.initial_items[].item_key` check (`commands/validate.rs` ~line 1181) — a typo'd
     `requires_item` should be a `missing_reference` error.
  2. A scene-load `warn!` in `scene_loader.rs`, mirroring the existing
     `warn_missing_player_stat_templates` convention, for the same check — needed because the CLI
     check silently doesn't run at all in a project with no `items_path` set, and a WASM-only
     designer (no local CLI access) would otherwise get no signal whatsoever before shipping a
     permanently-unopenable door (worse than `initial_items`' cosmetic-phantom-slot failure mode
     the CLI check's sibling was built for — this one fails *closed*, not just ugly).
- **Local co-op:** `PlayerInventory` is a single global resource, not per-player (unlike stat
  pools) — stated explicitly per system-architect's review, since it's easy to assume otherwise.
  In a 2+ player scene, any one player carrying the required item unlocks the door for everyone,
  consistent with every other inventory-reading action (`AddItem`/`RemoveItem`/`BuyItem` all
  already route the literal `"player"` entity id to this same shared resource) — this feature
  inherits that scope rather than introducing a new one. Document this in
  `docs/20_data_formats.md`.
- **Free side effect, worth documenting rather than hiding:** `dialogue_tick_system` runs
  `.after(interactable_system)` and auto-fires `StartDialogue` on `entity.interacted:{id}` for any
  entity with `PrefabDef.dialogue` set. Combining `dialogue:` with an item-gated `interactable:`
  means the gate applies to starting the conversation too — a real, useful combination (a locked
  gate NPC who won't even talk to you without the key), not an edge case to guard against.
- **No inventory-side change.** `PlayerInventory` stays the single global resource it is today —
  this feature doesn't touch per-player pools, and doesn't need to.

### RON example

```ron
// prefabs/prefabs.ron
"seal_door": (
    kind: Prop,
    model: "seal_door",
    interactable: (radius: 2.0, hint_text: "Open", requires_item: "old_key"),
),
```

```ron
// logic/state_machine.ron (or the entity's own .behavior.ron — same schema either way)
global_on: [
    ( event: "entity.interact_blocked:seal_door", do_actions: [
        ShowFloatingText(entity: "seal_door", text: "Locked - needs the old key"),
        PlaySound(key: "locked_door"),
    ] ),
],
```
(`PlaySound`'s `key:` must resolve in the project's `assets.ron` `audio:` catalog — pick an
existing key or add one; not spec'd here since it's asset-content, not engine work. Plain ASCII
only in any `text:`/`ShowFloatingText` string — the embedded font has no non-ASCII glyph coverage,
see `docs/20_data_formats.md`'s Label section.)

## Tasks
- [ ] `requires_item: Option<String>` on `InteractableDef` + `Interactable` component
- [ ] `has_item()` helper in `capabilities/inventory.rs`
- [ ] `interactable_system` gate check: `entity.interact_blocked:{id}` when missing, counts toward
      `hit_any` either way
- [ ] `ironhold_cli validate` reference check for `requires_item` (mirrors `initial_items` check)
- [ ] Scene-load `warn!` for a `requires_item` with no matching `items.ron` entry (mirrors
      `warn_missing_player_stat_templates`)
- [ ] Tests: blocked-when-missing fires `entity.interact_blocked`, not `entity.interacted`;
      interacted-when-present fires exactly as an ungated interactable does; blocked interact does
      NOT fire `player.attack_missed` (regression guard for the `hit_any` fix above); no
      `requires_item` authored at all behaves identically to today (regression guard); CLI validate
      + scene-load warn both catch a typo'd `requires_item`
- [ ] Shipped example: add a locked door to `3rd_person_game_demo` (prefab + an `items.ron` key +
      an `assets.ron` audio key + the `global_on:`/`.behavior.ron` wiring above) — the project
      already has `items/items.ron` and interactable prefabs, so it's the natural home; designers
      copy from shipped projects, not from feature-plan files
- [ ] Docs — all five surfaces this touches, not just the two most obvious ones:
  - `docs/20_data_formats.md`'s `interactable` schema-table row — while here, also fix the
    pre-existing gap where the row's field list only names `radius` despite `hint_text` already
    shipping (three worked examples in the same doc already use it undocumented)
  - `docs/30_runtime_events_and_logic.md`'s `entity.*` event list (~line 106)
  - `docs/30_runtime_events_and_logic.md`'s component → emitted-event table (~line 471)
  - `docs/STATUS.md`'s Interactable entities row (~line 57)
  - Local co-op note (shared-inventory-unlocks-for-everyone, see Approach above)

## Open questions
- None outstanding — both plan-review passes (system-architect, ux-gamedesigner-reviewer) raised
  concrete issues, all resolved above: the `hit_any`/`player.attack_missed` polarity was backwards
  in the first draft (fixed); the CLI-only diagnostic was insufficient for WASM-only designers
  (added the scene-load `warn!` layer); the worked RON example didn't actually parse (wrong
  behavior-file shape, wrong `PlaySound` syntax, a non-ASCII em-dash — all fixed above); the
  "no reason payload on the blocked event" design choice and the "no wildcard match" both-events
  behavior are now stated explicitly rather than left implicit.

## Acceptance criteria
- Given an `interactable` with `requires_item: "old_key"` and a player without `old_key`, when the
  player presses interact in range, then `entity.interact_blocked:{id}` fires, `entity.interacted:{id}`
  does not, and `player.attack_missed` does not fire either (the press landed on the door, it was
  correctly refused — not the same as pressing interact with nothing in range at all).
- Given the same setup but the player has `old_key` in `PlayerInventory`, then `entity.interacted:{id}`
  fires exactly as it does for a gate-less `interactable` today.
- Given no `requires_item` field authored at all, behavior is unchanged from today (regression
  guard — existing shipped projects using `interactable` must not need any RON change).
- Given a `requires_item` value with no matching `items.ron` entry, both `ironhold_cli validate`
  (a `missing_reference` error) and a scene-load `warn!` report it — the latter so a WASM-only
  designer with no CLI access still gets a signal before shipping a permanently-unopenable door.

# Feature: Hover/Focus Tooltips for Inventory Items & Action Bar Slots

_Status: Draft_
_Planned at: `de7a659` (2026-06-22)_

## What

Adds a floating, data-driven tooltip that appears when the player hovers an **inventory slot containing an item** or an **action bar slot**. The card shows designer-authored content — item name/description/weight/tags/stack, or skill label/description/cost/cooldown/effect summary — resolved from RON (`items.ron`, `ActionSlotDef`). No new Rust is needed to author tooltip text; designers add a `description` to an `ItemDef` and a `tooltip` block to an `ActionSlotDef`. The tooltip is a single shared Bevy UI overlay node, repositioned on hover change, edge-clamped to stay on screen, and despawned (hidden) on mouse-leave or panel close. It works identically on native and WASM.

## Why

The engine already has inventory, shop, container, and action-bar UI, but there is **no way for a designer to surface what an item or skill does** without baking it into a static label. `ActionSlotDef.label` was reserved months ago ("future use — tooltip label") and never wired. This feature closes that gap and is a prerequisite for any project that wants discoverable RPG-style UI. It also establishes the first reusable **hover → overlay** pattern in the codebase, which future features (equipment comparisons, status-effect hovers, world-object inspect) can build on.

## Why now / scope discipline

This is a self-contained, additive UI capability with **zero changes to the Message→Action pipeline**. A tooltip is a pure render-time reaction to hover state — like `target_indicator`, it is a cosmetic side-effect and must NOT push to `ActionQueue`. Keeping it pipeline-free is the right call; do not add tooltip Actions/Events.

## Approach

### New capability module: `capabilities/tooltip.rs`

A new `TooltipPlugin` owning:
- A `Tooltip` overlay resource (singleton entity handle).
- A hover-detection system (reads `Changed<Interaction>` on slot entities).
- A positioning/clamping system.
- A teardown hook on scene load / panel close.

It does not touch the interpreter, `ActionQueue`, or any executor arm.

### Schema changes

**`ItemDef` (`schema/items.rs`)** — add a description field:

```rust
/// Multi-line flavour/effect text shown in the hover tooltip. Omit for no body text.
/// Newlines (`\n`) are honoured. Keep concise — the card wraps at a fixed width.
#[serde(default)]
pub description: Option<String>,
```

`ItemDef` already derives `#[serde(deny_unknown_fields)]`, so the field must be added before any RON file references it. `#[serde(default)]` → existing `items.ron` files deserialize with `description: None` and render a name-only tooltip. No `schema_version` bump is required (additive optional field, backward compatible) — but note the version in the migration section so future readers know it was a v1-compatible add.

**`ActionSlotDef` (`schema/scene_v2.rs`)** — replace the unrendered `label` with a structured tooltip. See the naming resolution below. Net schema:

```rust
/// Hover tooltip content for this slot. Omit for no tooltip.
#[serde(default)]
pub tooltip: Option<SlotTooltipDef>,
```

New struct (kept alongside `ActionSlotDef`, `SlotCost`):

```rust
/// Designer-authored hover tooltip for an action bar slot.
/// All text is authored by hand — the engine never auto-generates a summary
/// from `do_actions` (that would couple UI copy to internal action variants).
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct SlotTooltipDef {
    /// Title line. Usually the skill name (e.g. "Fireball").
    pub title: String,
    /// Body text describing what the skill does. Newlines honoured. Optional.
    #[serde(default)]
    pub description: Option<String>,
    /// One-line effect summary shown beneath the body (e.g. "Deals 40 fire damage").
    /// Hand-authored — NOT derived from `do_actions`.
    #[serde(default)]
    pub effect: Option<String>,
}
```

Cost and cooldown are **not** duplicated into `SlotTooltipDef` — they already live on `ActionSlotDef.cost` / `ActionSlotDef.cooldown_secs`, and the tooltip system reads them directly. Duplicating them would create a drift hazard (the bar enforces one number, the tooltip displays another).

### Resolving `ActionSlotDef.label`

`label: Option<String>` currently exists, is documented "future use — tooltip label", and is **never rendered**. Resolution: **remove `label` and supersede it with `tooltip: Option<SlotTooltipDef>`.**
- A bare title string is too thin — skills need a description and an effect line, so a struct is the right shape from the start.
- `label` has zero production consumers (grep confirms it is read nowhere), so removing it is not a breaking change to any working behaviour. The only risk is a RON file that *sets* `label:` — `deny_unknown_fields` will then reject it. The migration step below sweeps the example projects for `label:` and rewrites them to `tooltip:`.
- Keeping both `label` and `tooltip` would leave two overlapping, half-documented fields — exactly the kind of schema cruft the schema-as-contract principle warns against.

Decision to confirm with Frank before coding: remove vs. keep-as-deprecated-alias. Recommendation: **remove** (clean, no consumers).

### Tooltip UI: single shared overlay entity (not spawn-per-hover)

One `Tooltip` overlay is spawned **once** at scene load (hidden), held in a resource, and **mutated** on hover change — its child text nodes are rewritten and `Visibility`/`Node.left`/`Node.top` updated. This satisfies the "not updated every frame" constraint without the churn of despawning/respawning a node tree on every hover (which would thrash Bevy's UI layout and, on WASM, risk repeated text-glyph atlas work).

```rust
#[derive(Resource, Default)]
pub struct LoadedTooltip {
    /// Root overlay node. Spawned hidden at scene load; despawned on LoadScene.
    pub overlay: Option<Entity>,
    /// The slot entity currently being described, to suppress redundant rebuilds.
    pub active_source: Option<Entity>,
}
```

- **Z-order:** the overlay is a **root-level UI node** (not a child of any panel) with a high `GlobalZIndex` (e.g. `GlobalZIndex(1000)`) so it floats above inventory/shop/container panels regardless of their spawn order. UI renders on the existing `Camera2d` at `order: 1000`; the overlay needs no separate camera.
- **Content rebuild is guarded:** the hover system only rebuilds the text children when `active_source` changes (i.e. the hovered slot actually changed), per the change-detection discipline in the core rules.
- **Tagged `LevelEntity`** so it is torn down on scene change like all other scene UI; `LoadScene` clears `LoadedTooltip`.

### Hover detection

Reuse Bevy `Interaction` via `Changed<Interaction>`, mirroring `button_system`.

- **Action bar slots already carry `Button`** (`scene_loader.rs`, the `ActionBar` arm) → `Interaction` is available with no change.
- **Inventory slots do NOT carry `Button`** (the `InventoryPanel` arm spawns slots with only `InventorySlotMarker`). Add `Button` to the inventory slot spawn so `Interaction` is generated. This is the one structural change to existing spawn code. (Shop/container slots are out of scope for v1 — see below — so leave them untouched.)

A single `tooltip_hover_system` queries both source types:
- `Query<(Entity, &Interaction, &InventorySlotMarker), Changed<Interaction>>`
- `Query<(Entity, &Interaction, &ActionSlotUi), Changed<Interaction>>`

On `Interaction::Hovered`: resolve content (inventory → `PlayerInventory` slot → `ItemDef`; action bar → `ActionSlotUi` + the slot's `SlotTooltipDef`), rebuild the overlay text, set `active_source`, make visible. On `Interaction::None` for the `active_source` entity: hide the overlay and clear `active_source`. An empty inventory slot produces no tooltip (hover on `None` content → stay hidden).

System ordering: run `.after(button_system)` in `Update` (UI hover state is settled by then), pipeline-free.

### Positioning + edge-clamping

A lightweight `tooltip_follow_system` (runs only while the overlay is visible) reads `PrimaryWindow` cursor position (the WASM-safe pattern already used in `targeting.rs`) and the window size, then sets the overlay's absolute `Node.left/top`:

- Default: place the card down-and-right of the cursor with a small offset (e.g. `+12, +12`).
- **Horizontal clamp:** if `cursor.x + offset + card_width > window.width()`, flip to the left of the cursor (`cursor.x - offset - card_width`).
- **Vertical clamp:** if `cursor.y + offset + card_height > window.height()`, flip above the cursor.
- Final clamp both axes to `>= 0` so the card never leaves the viewport top-left.

Card width is fixed (designer-independent, e.g. 260 px) so clamping math needs no layout read. Card height can be measured from the laid-out `ComputedNode` after the first frame, or approximated from line count for the flip decision (approximation is fine — the final `max(0.0)` clamp guarantees on-screen). Viewport size comes from `Window`, which Bevy populates correctly under WASM (canvas size), so no `web-sys` call is needed — keeps `ironhold_core` platform-agnostic.

### Content layout

- **Item tooltip:** title (`display_name`) · description (if any) · a stat line built from `weight`, stack info (`count`/`max_stack` when stackable), and `tags` joined as a dim footer. `currency_stat` items are looted directly and never sit in a slot, so they will not normally be hovered — no special-casing needed.
- **Skill tooltip:** `tooltip.title` · `tooltip.description` · `tooltip.effect` · a cost/cooldown footer assembled from `ActionSlotUi.cost` (`"{amount} {stat}"`) and `cooldown_secs` (`"{n}s cooldown"`). Footer omitted when both are absent.

All colours/sizes for the card come from sensible engine defaults for v1 (no new catalog entries). If designers later want themed cards, a `TooltipStyleDef` can be added to the scene — explicitly deferred.

## Tasks

- [ ] Add `description: Option<String>` to `ItemDef` (`schema/items.rs`) with doc comment + `#[serde(default)]`.
- [ ] Add `SlotTooltipDef` struct and `tooltip: Option<SlotTooltipDef>` to `ActionSlotDef`; **remove** `label`. (`schema/scene_v2.rs`)
- [ ] Create `capabilities/tooltip.rs`: `TooltipPlugin`, `LoadedTooltip` resource, overlay spawn-at-scene-load (hidden, `GlobalZIndex`, `LevelEntity`).
- [ ] `tooltip_hover_system` — `Changed<Interaction>` over inventory + action-bar slots; rebuild content on `active_source` change; guarded writes.
- [ ] `tooltip_follow_system` — cursor-follow + edge-clamp via `PrimaryWindow`; runs only while visible.
- [ ] Add `Button` to the inventory slot spawn in `scene_loader.rs` `InventoryPanel` arm (so `Interaction` exists). Confirm it does not alter slot visuals (no `BackgroundColor` swap system targets `InventorySlotMarker`).
- [ ] Clear `LoadedTooltip` on `Action::LoadScene` (alongside the other UI resource clears).
- [ ] Register `TooltipPlugin` in the core plugin set.
- [ ] Carry the new schema fields through `ironhold_cli` `query` if it surfaces item/slot fields (run `cargo check -p ironhold_cli`; verify `query actions` / item queries still parse).
- [ ] Migrate example RON: add a `description` to a few items in a demo project and a `tooltip` to action-bar slots in `3rd_person_game_demo`; sweep for any `label:` on slots and rewrite to `tooltip:`.
- [ ] Tests (integration): item-slot hover shows item card; action-slot hover shows skill card; empty slot shows nothing; mouse-leave hides; `LoadScene` despawns overlay. Add a `ron_validation` case for an item with `description` and a slot with `tooltip`.
- [ ] Docs: `docs/20_data_formats.md` (`ItemDef.description`, `SlotTooltipDef`, removal of `label`), `crates/ironhold_core/src/CLAUDE.md` (new pipeline-free cosmetic capability + the inventory-slot `Button` requirement).
- [ ] WASM dev build + browser play-test (hover near right/bottom edges to verify flip).

## RON examples

Item with a description (`items/items.ron`):

```ron
"health_potion": (
    display_name: "Health Potion",
    description: "Restores 50 HP over 3 seconds.\nCannot be used in combat.",
    icon_index: 4,
    stackable: true,
    max_stack: 20,
    weight: 0.5,
    tags: ["consumable", "healing"],
),
```

Action slot with a tooltip (inside an `ActionBar` UI node in a `.scene.ron`):

```ron
ActionSlotDef(
    key: "1",
    icon_index: 2,
    cooldown_secs: Some(6.0),
    cost: Some((stat: "mana", amount: 25.0)),
    tooltip: Some((
        title: "Fireball",
        description: "Hurls a bolt of fire at the current target.",
        effect: "Deals 40 fire damage",
    )),
    do_actions: [
        ShowDamagePopup(entity: "{target}", amount: -40.0),
        ModifyStat(key: "{target}.health", delta: -40.0),
    ],
),
```

(The displayed cost "25 mana" and "6s cooldown" footer are read from `cost`/`cooldown_secs`, not authored twice in `tooltip`.)

## Migration

- **`ItemDef.description`** — optional, `#[serde(default)]`. Every existing `items.ron` deserializes unchanged with `description: None` → name-only tooltip. `ITEM_CATALOG_SCHEMA_VERSION` stays at `1` (additive optional field is backward compatible).
- **`ActionSlotDef.label` → `tooltip`** — `label` is read by no system, so removing it changes no behaviour. Risk: a RON file that *sets* `label:` will fail `deny_unknown_fields`. Mitigation: grep the repo for `label:` within `ActionSlotDef` blocks before merging; rewrite each to a `tooltip:` block. The CLI validator (`cargo run -p ironhold_cli -- validate <project>`) will flag any missed occurrence as a parse error, so the sweep is verifiable.
- **Inventory slot `Button` addition** — purely additive component; no RON or save-data impact.

## Out of scope (v1)

- **Shop and container slot tooltips** — same overlay can extend to them later, but v1 wires only inventory + action bar. Shop already shows a price; container slots are transient.
- **Gamepad/keyboard focus tooltips** — hover-only for v1. Focus-driven tooltips need a focus model that does not yet exist.
- **Animated/fading tooltips** — appears/disappears instantly per the constraint.
- **Item comparison tooltips** (equipped vs. hovered side-by-side) — needs an equipment concept that does not exist yet.
- **Auto-generated effect summaries from `do_actions`** — explicitly rejected; effect text is hand-authored to avoid coupling UI copy to internal action variants.
- **Themed/per-scene tooltip styling** (`TooltipStyleDef`) — engine-default styling only for v1; deferred until a project needs it.
- **Rich content** (icons/images inside the card, embedded stat bars) — text-only card for v1.

## Decisions

- **`ActionSlotDef.label` removal:** confirmed — remove outright. No consumers; superseded by `tooltip: Option<SlotTooltipDef>`.
- **`button_system` / inventory hover tint:** `button_system` filters `With<UiAction>`; inventory slots have no `UiAction`, so adding `Button` does not give them the hover tint. Confirmed safe.
- **Card width:** fixed 260 px for v1.

## Acceptance criteria

- Given an inventory slot holding an item with a `description`, when the player hovers it, then a tooltip card appears within ~1 frame showing name, description, weight, stack, and tags.
- Given an inventory slot that is empty, when the player hovers it, then no tooltip appears.
- Given an action bar slot with a `tooltip`, when the player hovers it, then a card appears showing title, description, effect, and a cost/cooldown footer derived from the slot's own `cost`/`cooldown_secs`.
- Given a visible tooltip, when the cursor leaves the slot, then the tooltip hides immediately (same or next frame).
- Given a slot near the right or bottom viewport edge, when its tooltip shows, then the card flips so it remains fully on screen (verified native and WASM).
- Given a visible tooltip, when a `LoadScene` action fires, then the overlay entity is despawned and `LoadedTooltip` is cleared (no orphaned overlay).
- Given existing `items.ron` files with no `description` field, when the project loads, then they parse successfully and produce name-only tooltips.
- The tooltip system pushes nothing to `ActionQueue` and adds no new `Action`/event variants.

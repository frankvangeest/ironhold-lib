# Feature: Targeting System (Click-to-Select + Tab Targeting)

_Status: DONE (shipped 2026-06-08)_
_Planned at: `52cfa02` (2026-06-02)_

> **Implemented-as note (deviations from this plan):**
> - **Click selection is screen-space proximity** (`camera.world_to_viewport`, nearest entity within ~70px), **not** `bevy::picking` mesh raycast. Mesh picking raycasts bind-pose geometry and misses animated/skinned GLB characters, so it was abandoned mid-implementation.
> - **No hover events** (`target.hovered`/`target.unhovered`) and **no `ProjectDecal` selection ring** were shipped — deferred as polish. Selection feedback is via the `target_display` HUD label + `ShowFloatingText`.
> - Added beyond plan: `PrefabKey` component and `target_display`/`target_name`/`target_id` HUD variables; `SetTarget`/`ClearTarget` actions.
> - Fixed two pre-existing spawn-path bugs surfaced by this work: GLB player missing `SpeedMultiplier` (movement) and GLB scene actors missing `SpawnId` (id-targeting). Follow-ups logged in `claude_suggestions.md` (spawn-site consolidation; `PrefabKey` on dynamic spawns).

---

> ## Pre-implementation checklist
>
> - [ ] **Decide: `click_selectable` vs `targetable` — keep them separate.** `click_selectable: true` opts an entity into mouse picking (friendly NPCs, interactable props). `targetable: true` opts an entity into Tab cycling (combat targets). A friendly NPC shopkeeper should be `click_selectable` but not `targetable`. These are two independent flags on `PrefabDef` — do not merge them into one.
>
> - [ ] **Decide: `CurrentTarget` as a global resource.** `CurrentTarget(Option<String>)` where the inner value is a `SpawnId` string. Single global resource — not per-player, not a component. This is correct for single-player and simplifies the substitution pass. Multiplayer will need to revisit but that's out of scope here.
>
> - [ ] **Decide: selection indicator rendering method for Tab targeting.** Three options: (a) `ProjectDecal` circle below entity (reuses existing infrastructure, no new meshes); (b) a thin ring mesh spawned as a child; (c) screen-space outline (requires post-process — not viable on WASM). Recommendation: `ProjectDecal` with a dedicated `"targeting/selection_ring"` texture key in `assets.ron`. Designer controls the texture; no new systems required beyond moving the decal each frame.
>
> - [ ] **Decide: hover visual for click-to-select.** Options: (a) cursor change only (no entity-side feedback); (b) a tint on the hovered mesh; (c) emit the event and let the designer wire a highlight via rules. Recommendation: emit `target.hovered:{id}` into the pipeline only — let the designer attach a visual response (e.g. `ProjectDecal` or `SetEntityVisible` on a highlight child). Zero engine-side visual coupling.
>
> - [ ] **Confirm `bevy::picking` + `MeshPickingPlugin` are not already registered.** `MeshPickingPlugin` must be added to the app in `lib.rs` alongside `MeshPickingCamera` on all spawned cameras. Verify there are no conflicts with the existing `bevy_rapier3d` physics colliders — `MeshPickingPlugin` raycasts against mesh geometry, not physics colliders. Both can coexist.
>
> - [ ] **Decide: Tab targeting sort order.** Nearest-first (distance from player entity) is the most game-friendly default. Wrap-around at list end. `Shift+Tab` reverses. Configurable faction filter: `hostile_only` (default), `all`. Declare as fields in the player prefab's input config block.

---

## What

Two related features that share a `CurrentTarget` resource and `{target}` substitution in the action pipeline:

**Click-to-select** — left-clicking a 3D entity that has `click_selectable: true` sets it as `CurrentTarget`. Hover emits `target.hovered:{id}`. Clicking empty space clears the target. Uses `bevy::picking` (mesh raycast, no physics).

**Tab targeting** — pressing Tab cycles through nearby `targetable: true` entities nearest-first. `Shift+Tab` reverses. Selection shown via a decal ring below the target. Both features update the same `CurrentTarget` resource, so either method can set the target and the other reads it.

---

## Why

Without a target, `{target}` substitution in behavior files and skill action bar slots resolves to nothing — the feature is wired but unusable. Tab targeting and click-to-select are the two standard input methods for setting a target. They share infrastructure so designing them together avoids contradictory assumptions about `CurrentTarget` later.

Unblocks: Skill action bar `{target}` substitution, AoE ground targeting, Dialogue initiation, Status effect icon display on the target.

---

## Shared infrastructure

### `CurrentTarget` resource (`runtime/scene_manager/mod.rs` or `capabilities/targeting.rs`)

```rust
#[derive(Resource, Default, Clone)]
pub struct CurrentTarget {
    pub spawn_id: Option<String>,   // SpawnId string of the selected entity
}
```

Cleared on `Action::LoadScene` (scene transition clears target).

### New actions

```ron
SetTarget("spawn_id")     // explicitly set from RON rules or behavior files
ClearTarget               // explicitly clear
```

### New events into the pipeline

```ron
target.changed:{spawn_id}       // new entity selected (either method)
target.cleared                  // target cleared (click empty space, ClearTarget action, scene load)
target.hovered:{spawn_id}       // cursor over a click_selectable entity
target.unhovered:{spawn_id}     // cursor left a click_selectable entity
target.clicked:{spawn_id}       // confirmed click on a click_selectable entity
```

### `{target}` substitution

`message_interpreter.rs` already handles `{self}` substitution. Extend it to resolve `{target}` from `CurrentTarget.spawn_id` in the same pass. If `CurrentTarget` is `None` and an action uses `{target}`, emit `action_bar.no_target:{slot}` (or silently skip for non-bar actions) — see skill action bar feature file for the slot-specific case.

---

## Click-to-Select

### New `PrefabDef` field

```ron
// prefabs.ron
"goblin_guard": (
    kind: "actor",
    model: "creatures/goblin_guard",
    click_selectable: true,   // NEW — opt in to mouse picking
    // ...
)
```

### How it works

1. `MeshPickingPlugin` + `MeshPickingCamera` added to the app (one-time setup in `lib.rs` + camera spawns).
2. At entity spawn time, if `click_selectable: true`, attach `Pickable::default()` component and register observers:
   - `On<Pointer<Click>>` → sets `CurrentTarget`, emits `target.clicked:{id}` + `target.changed:{id}`
   - `On<Pointer<Over>>` → emits `target.hovered:{id}`
   - `On<Pointer<Out>>` → emits `target.unhovered:{id}`
3. A global `pointer_miss_system` runs each frame: if `MouseButton::Left` was just released AND no picking hit was registered this frame (check via `PointerHits` or a frame flag set by the observers), clear `CurrentTarget` and emit `target.cleared`.

### New Rust changes

- `lib.rs` — add `MeshPickingPlugin` to the app.
- `scene_loader.rs` — add `MeshPickingCamera` to all three camera spawns (orbit, flycam, default).
- `entity_spawner.rs` — at spawn time, insert `Pickable::default()` and register the three observers when `click_selectable: true`.
- `capabilities/targeting.rs` (new file) — `CurrentTarget` resource, `pointer_miss_system`, `SetTarget`/`ClearTarget` executor arms.
- `schema/catalog.rs` (or `prefabs.rs`) — add `click_selectable: bool` (default false) to `PrefabDef`.
- `runtime/scene_manager/action_executor.rs` — handle `SetTarget`, `ClearTarget`.
- `runtime/scene_manager/message_interpreter.rs` — add `{target}` substitution.

---

## Tab Targeting

### New `PrefabDef` field

```ron
"orc_enemy": (
    kind: "actor",
    model: "creatures/orc",
    targetable: true,     // NEW — opt in to Tab targeting
    // ...
)
```

### New input config fields (player prefab)

```ron
inputs: (
    // ... existing keys ...
    target_next: "Tab",          // cycle to next target (default Tab)
    target_prev: "ShiftTab",     // cycle to previous target
    target_filter: "HostileOnly", // HostileOnly | All
    target_range: 30.0,          // max distance (world units) to consider
)
```

### How it works

1. Entities with `targetable: true` get a `Targetable` marker component at spawn.
2. `tab_targeting_system` runs in `Update`, reads player position, detects `target_next` / `target_prev` key press:
   - Collects all `Targetable` entities within `target_range` of the player.
   - Sorts by distance (nearest first).
   - Applies faction filter (if `HostileOnly`, exclude entities without `NpcAgent` or with a friendly faction — faction system deferred to Group system; for v1, `HostileOnly` means `has NpcAgent`).
   - Cycles index: wraps at both ends.
   - Sets `CurrentTarget`, emits `target.changed:{id}`.
3. If the currently targeted entity is despawned, `CurrentTarget` is cleared on the next `tab_targeting_system` tick.

### Selection indicator

A `ProjectDecal` entity is maintained as a persistent "selection ring" scene entity, initially invisible (`Visibility::Hidden`). When `CurrentTarget` changes to `Some(id)`, the indicator system:
1. Resolves the entity via `SpawnRegistry`.
2. Moves the decal to the entity's `GlobalTransform` position (y-offset +0.05 to avoid z-fighting).
3. Sets `Visibility::Visible`.

When `CurrentTarget` is cleared, sets `Visibility::Hidden`.

The ring texture (`"targeting/selection_ring"`) is an asset catalog key — designer controls the visual.

```ron
// scene RON — optional override per scene
selection_indicator: (
    texture: "targeting/selection_ring",
    radius: 1.2,
    color: (0.2, 0.8, 1.0, 0.8),
)
```

If no `selection_indicator` block is present in the scene, a default ring is used.

### New Rust changes

- `schema/catalog.rs` (or `prefabs.rs`) — add `targetable: bool` (default false) to `PrefabDef`.
- `schema/player.rs` — add `target_next`, `target_prev`, `target_filter`, `target_range` to the input config struct.
- `capabilities/targeting.rs` — `Targetable` component, `tab_targeting_system`, `selection_indicator_system`.
- `entity_spawner.rs` — insert `Targetable` component when `targetable: true`.

---

## `{target}` in the action pipeline

Once `CurrentTarget` is set, `{target}` substitutes in:
- Behavior file action strings: `SpawnEffect(key: "hit_spark", entity: "{target}")`
- Skill action bar `do_actions`: `ModifyStat("{target}.health", -25.0)`
- Rule `do_actions`: `EmitEvent("npc.aggro:{target}")`

The substitution pass in `message_interpreter.rs` resolves `{target}` in the same place as `{self}`, using `CurrentTarget.spawn_id`. If `None`, the literal string `"{target}"` is left in place (a warning is logged).

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `CurrentTarget` resource in `capabilities/targeting.rs`
- [ ] `click_selectable: bool` on `PrefabDef`
- [ ] `targetable: bool` on `PrefabDef`
- [ ] `target_next`, `target_prev`, `target_filter`, `target_range` on player input config
- [ ] `MeshPickingPlugin` added to app in `lib.rs`
- [ ] `MeshPickingCamera` added to all camera spawns in `scene_loader.rs`
- [ ] Spawn-time: `Pickable` + observers for `click_selectable` entities
- [ ] `pointer_miss_system` — clear target on click-on-nothing
- [ ] `tab_targeting_system` — cycle through `Targetable` entities
- [ ] `selection_indicator_system` — move/show/hide `ProjectDecal` ring
- [ ] `SetTarget` / `ClearTarget` actions in executor
- [ ] `{target}` substitution in `message_interpreter.rs`
- [ ] `target.*` events emitted into pipeline for all state changes
- [ ] `CurrentTarget` cleared on `LoadScene`
- [ ] Demo: add `targetable: true` to enemies in `primitive_world` or `3rd_person_game_demo`; wire a rule that fires on `target.changed`
- [ ] Integration tests: click selectable → `CurrentTarget` set; Tab cycles nearest-first; click empty space → cleared; despawned entity auto-clears
- [ ] Docs: `click_selectable`, `targetable` fields in `docs/20_data_formats.md`; `{target}` substitution in `docs/30_runtime_events_and_logic.md`

---

## Open questions

- **Faction filter for v1**: without the Group system, "hostile only" is approximated by `has NpcAgent`. This is a reasonable v1 heuristic but will need to be replaced when Group system ships. Document the approximation clearly.
- **Multiple pointers / touch**: `bevy::picking` supports multiple pointers. For v1, only respond to `PointerId::Mouse`. Touch support deferred.
- **Click-to-select and UI overlap**: `bevy::picking` will fire through UI elements unless blocked. Ensure `Pickable` observers don't fire when the click lands on a UI panel (stat bar, action bar, etc.). Bevy 0.18 picking has a `should_block_lower` setting on UI nodes — verify it works for this case.
- **Tab targeting and no valid targets**: if there are no `Targetable` entities in range, pressing Tab should be a no-op (no event, no sound). Log a debug message.
- **`{target}` with no target set**: for non-action-bar contexts (rules.ron, behavior files), silently leave `{target}` unsubstituted and log a warning. The action bar already handles this with `action_bar.no_target:{slot}`.

---

## Acceptance criteria

- Given an entity with `click_selectable: true`, left-clicking it sets `CurrentTarget` to its `SpawnId` and emits `target.clicked:{id}` and `target.changed:{id}` into the pipeline.
- Given `CurrentTarget` is set and the player left-clicks empty space, `CurrentTarget` is cleared and `target.cleared` is emitted.
- Given `Pointer<Over>` on a `click_selectable` entity, `target.hovered:{id}` is emitted. `target.unhovered:{id}` fires when the cursor leaves.
- Given entities with `targetable: true` within range, pressing Tab cycles to the nearest one, sets `CurrentTarget`, and emits `target.changed:{id}`.
- Given Tab pressed again, `CurrentTarget` advances to the next-nearest; at the end of the list it wraps to the first.
- Given `Shift+Tab`, cycling reverses.
- Given the currently targeted entity is despawned, `CurrentTarget` is cleared within one frame.
- Given a rule with `do_actions: [EmitEvent("selected:{target}")]` and `CurrentTarget` set, the emitted event contains the actual spawn ID.
- Given `Action::ClearTarget` in a rule, `CurrentTarget` is set to `None` and `target.cleared` is emitted.
- Given a scene transition (`LoadScene`), `CurrentTarget` is cleared.

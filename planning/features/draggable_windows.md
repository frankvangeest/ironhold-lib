# Feature: Draggable UI Windows

_Status: Draft_
_Planned at: `de7a659` (2026-06-22)_

## What
Lets the player reposition the three movable UI windows — the Player Inventory panel,
the Shop window, and the Container (chest) panel — by clicking and holding the left mouse
button on the window's title bar and dragging. The window follows the cursor in real time
and is clamped so no part leaves the viewport. Releasing the mouse drops the window in place.
Drag is purely a positional convenience: it does not push to the `ActionQueue`, touches no
RON or save data, and has no gameplay effect. Designers keep authoring the window's *initial*
on-screen position in RON exactly as they do today (the existing `position` field on each
panel def); drag only changes the runtime position from that starting point.

## Why
All three panels are absolutely positioned at fixed RON coordinates and can overlap each
other and obscure the 3-D scene. Once two of them can be open at once (inventory + shop, or
inventory + container) the fixed layout becomes a problem — the player cannot see both at
once without the designer hand-tuning non-overlapping positions per scene. Drag-to-reposition
is the standard ARPG/inventory-UI affordance and removes that authoring burden. It is also a
reusable primitive: any future absolutely-positioned panel (character sheet, spellbook, quest
log) gets dragging for free by reusing the same title-bar marker + drag system.

## Current panel structure (what I found)

All three panels are spawned by `spawn_ui_element_node` in
`runtime/scene_manager/scene_loader.rs` (arms `UiNodeDef::InventoryPanel` ~1849,
`UiNodeDef::ShopPanel` ~2076, `UiNodeDef::ContainerPanel` ~2166).

- **Absolutely positioned — yes.** The `node` handed to each arm is built at the call site
  (scene_loader.rs ~1368 for absolute mode / ~1327 panel mode) with
  `position_type: PositionType::Absolute`, `left: Val::Px(el.position().0)`,
  `top: Val::Px(el.position().1)`. The panel arm reuses that node (`let mut panel_node = node;`)
  and only overrides `flex_direction`/padding, so **`left`/`top` live on the panel root entity's
  `Node`** — exactly the fields a drag system needs to mutate.

- **Title bars exist — yes, as child nodes, but with NO marker and NO `Button`/`Interaction`.**
  Each panel's first child is a header row:
  - Inventory → `Name::new("InvHeader")` (scene_loader.rs ~1922)
  - Shop → `Name::new("ShopHeader")` (~2097)
  - Container → `Name::new("ContainerHeader")` (~2220)
  These are plain `Node` + `BackgroundColor` rows containing the title `Text` and the close
  `Button`. They are **not** tagged with any marker component and do **not** carry Bevy's
  `Button` (so they have no `Interaction`). This is the main wiring gap — see Task 2.

- **Panel root entities are tracked in resources.**
  `LoadedInventoryUi.inventory_panel: Option<Entity>` and `.shop_panel`, plus
  `LoadedContainerUi.container_panel: Option<Entity>` (inventory.rs ~38/52) already hold the
  root entity of each panel. Useful for clamping/z-order but the drag system can also reach the
  root via the title bar's `DragHandle { panel }` (see Architecture) without consulting these.

- **RON `position` field — already present on all three.** `InventoryPanelDef.position`,
  `ShopPanelDef.position`, `ContainerPanelDef.position` are all `(f32, f32)` "top-left corner in
  pixels (always absolute)" (scene_v2.rs ~922 / ~987 / ~1031). **No new RON field is required.**
  Drag mutates the runtime `Node.left/top`; the RON value is just the starting position.

- **Cursor + viewport access (WASM-safe).** Established pattern in `targeting.rs`
  (`click_select_system`): `Query<&Window, With<PrimaryWindow>>` then `window.cursor_position()`
  and `window.size()`. Works identically native + WASM (Bevy abstracts windowing — no platform
  API). Cursor position and UI `left`/`top` share the same top-left-origin logical-pixel space,
  so `new_left = cursor.x - grab_offset.x` is a direct mapping with no coordinate conversion.

- **UI camera / z-order.** UI renders on a `Camera2d` at `order: 1000`. Within it, sibling paint
  order stacks later-spawned nodes on top; for cross-panel "bring to front" use Bevy
  `GlobalZIndex` on the panel root (see Z-order task — optional).

## Approach

A single small capability module (`capabilities/window_drag.rs`) plus one marker component on
each title bar. No schema changes, no new actions, no `ActionQueue` involvement.

### Drag state — resource, not per-entity component
Use one `WindowDragState` **resource**, not a component on each panel:

```rust
#[derive(Resource, Default)]
pub struct WindowDragState {
    /// The panel root entity currently being dragged, plus the cursor-to-panel-top-left
    /// offset captured at pick-up time. `None` when nothing is being dragged.
    active: Option<DragSession>,
}

struct DragSession {
    panel: Entity,        // panel root whose Node.left/top we mutate
    grab_offset: Vec2,    // cursor_pos - panel_top_left at the moment of press
}
```

Rationale for a resource over a per-panel component: **at most one window can be dragged at a
time** (one cursor, one left button). The "two windows open at once" requirement is about
*independent position state*, which is already satisfied — each panel owns its own `Node.left/top`.
A singleton drag session that names the active panel is simpler than scanning N panels for a
"currently dragging" flag and cannot suffer cross-contamination because only one `panel` entity
is ever referenced. (If we later want multi-pointer/touch we revisit, but that is out of scope.)

### Title-bar detection — `DragHandle { panel: Entity }` marker + `Button`
The cleanest identification is an explicit back-pointer rather than walking the hierarchy:

```rust
#[derive(Component)]
pub struct DragHandle { pub panel: Entity }
```

Insert `DragHandle { panel: <panel root entity> }` **and** Bevy's `Button` on each header node so
it gains an `Interaction`. The panel root `Entity` is known inside each arm (it is the `.id()`
captured into `inventory_ui.inventory_panel` etc.), but the header is spawned *inside*
`with_children` before that `.id()` is returned. Two clean ways to wire it:

1. **Capture the root id, then insert on the header afterward.** After
   `let entity = parent.spawn(...).with_children(...).id();`, the header child is already spawned;
   add a `DragHandleTarget` transient marker on the header during `with_children`, then in a tiny
   follow-up `commands.entity(header).insert(DragHandle { panel: entity })`. Slightly awkward.
2. **Preferred: detect by header marker in the drag system, resolve panel via `ChildOf`/`Parent`.**
   Tag the header with a zero-field `WindowTitleBar` marker (+ `Button`) inside `with_children`,
   and in the drag system read the header's parent (`&ChildOf`) to get the panel root. This avoids
   needing the root `Entity` at header-spawn time entirely. Confirm the header is a *direct* child
   of the panel root (it is, in all three arms) so one parent hop suffices.

Recommend option 2 — it keeps the scene_loader change to a one-tuple insert per arm and needs no
post-spawn `commands` dance.

### Pick-up / drag / release — one `Update` system
`window_drag_system` (in `Update`, alongside `button_system`):

- **Pick-up:** on `mouse.just_pressed(Left)`, for each `WindowTitleBar` with `Interaction::Pressed`
  (or `Hovered` — see open question), resolve its panel root via `ChildOf`, read the panel's
  current `Node.left/top` (resolve `Val::Px`), capture `grab_offset = cursor - panel_top_left`,
  store the `DragSession`.
- **Drag:** while `mouse.pressed(Left)` and `active.is_some()`, set
  `panel.Node.left = Px(clamp_x(cursor.x - grab_offset.x))`,
  `panel.Node.top = Px(clamp_y(cursor.y - grab_offset.y))`. Guard the write (only set when the
  value actually changes by ≥ ~0.5 px) per the change-detection discipline in
  `core/src/CLAUDE.md` — unconditional `Node` writes re-trigger UI layout every frame.
- **Release:** on `mouse.just_released(Left)`, clear `active`.

### Mouse-delta / no-jump
Storing `grab_offset` at pick-up and computing `cursor - grab_offset` every frame (rather than
accumulating frame-to-frame deltas) guarantees the window does not jump on click and stays glued
to the same point under the cursor for the whole drag. This is the standard offset approach and
needs no `MouseMotion` event reader.

### Viewport clamping — needs rendered size
Clamp so `0 <= left <= viewport_w - panel_w` and `0 <= top <= viewport_h - panel_h`.

- Viewport: `window.size()` (WASM-safe, from `PrimaryWindow`).
- Panel rendered size: read the panel root's **`ComputedNode`** (`computed_node.size()` gives the
  laid-out px size including children/padding) rather than the authored `Node` width/height —
  inventory/container panels size to their slot grid via flex and have no explicit width, so the
  authored `Node` has `Val::Auto` and cannot be used for clamping. `ComputedNode` is populated by
  Bevy's UI layout pass and is available the same way on native and WASM. If `ComputedNode` is not
  yet ready on the very first frame (size 0), skip clamping that frame (the panel was just spawned
  and is not being dragged yet anyway).

### Z-order on drag (optional, recommended)
When two panels overlap, the dragged one should come to the front. Cheapest correct approach:
on pick-up, set `GlobalZIndex(1)` on the active panel root and reset previously-raised panels to
`GlobalZIndex(0)`. Without this, stacking follows spawn order (inventory < shop < container by
scene_loader order) and a dragged inventory could slide *under* the shop. Keep this minimal — a
single "raised" z-index, not a full window-manager focus stack.

### Why this respects the pipeline
Dragging is a pure positional mutation of a `Node`, triggered by raw mouse input, with no gameplay
consequence — exactly the category the `core/src/CLAUDE.md` pipeline rules carve out for cosmetic
side-effects (cf. `target_indicator_system`, which also mutates transforms directly and does NOT
go through `ActionQueue`). The drag system therefore reads `ButtonInput<MouseButton>` + `Window`
and writes `Node`/`GlobalZIndex` directly. It must **never** take `ResMut<ActionQueue>` or emit a
`UiEvent`/`GameEvent` for the move itself.

## RON impact
**None required for the drag mechanic.** All three panel defs already expose
`position: (f32, f32)` as the designer-authored initial top-left, and the runtime drag simply
moves the window from there. Drag state is entirely runtime (`WindowDragState` resource) and is
intentionally not persisted to RON or save data.

Existing RON stays valid and unchanged, e.g.:

```ron
InventoryPanel((
    id: "inventory_panel",
    position: (20.0, 20.0),   // initial top-left; player can drag it from here
    columns: 5,
    rows: 4,
    slot_size: 48.0,
))
```

(No `draggable: bool` toggle is proposed — all three windows are draggable by virtue of having a
title bar. If a non-draggable panel is ever needed, add an opt-out flag then; do not pre-build it.)

## Tasks
- [ ] Add `capabilities/window_drag.rs`: `WindowDragState` resource, `DragSession`,
      `WindowTitleBar` (+ `DragHandle`/parent-resolution choice), `WindowDragPlugin`.
- [ ] Register the plugin and add `window_drag_system` to `Update` in `lib.rs` (near `button_system`).
- [ ] Tag the three header nodes in `scene_loader.rs` (`InvHeader` ~1922, `ShopHeader` ~2097,
      `ContainerHeader` ~2220) with `WindowTitleBar` + `Button` so they gain `Interaction`.
      Confirm each header is a direct child of its panel root.
- [ ] Implement pick-up (capture `grab_offset`), drag (`cursor - grab_offset`, guarded `Node` write),
      and release in `window_drag_system`.
- [ ] Viewport clamp using `window.size()` + the panel root's `ComputedNode.size()`; skip the frame
      if `ComputedNode` is not yet populated.
- [ ] Z-order: raise the dragged panel via `GlobalZIndex` on pick-up; reset others.
- [ ] Verify the close `Button` inside each header still works (drag must not swallow the close
      click — the close button is a nested `Button`, so `Interaction::Pressed` on it should not also
      start a drag of the header; confirm child-button presses do not register as title-bar presses).
- [ ] Tests: integration test in `assets/projects/integration_tests/` that opens a panel, simulates
      a press-on-title-bar → cursor move → release, and asserts `Node.left/top` changed and stayed
      within viewport bounds; assert no `Action` was pushed to `ActionQueue` during the drag.
- [ ] Docs: note the drag affordance and that `position` is the *initial* position in
      `docs/20_data_formats.md` (InventoryPanel/ShopPanel/ContainerPanel sections) and add a line to
      `crates/ironhold_core/src/CLAUDE.md` listing `window_drag_system` as a pipeline-exempt cosmetic
      system (alongside `target_indicator_system`).

## Open questions
- **Touch / multi-pointer on WASM mobile** — out of scope for v1 (single left-button only)?

## Decisions
- **Pick-up trigger: `Interaction::Pressed` on header (Option A).** Both architect and UX reviewer
  agree. Architect confirms Bevy's `ui_focus_system` sets `Interaction::Pressed` only on the
  topmost `Button` under the cursor — the nested close `Button` naturally blocks the header drag
  for free via `FocusPolicy::Block`, with no manual hit-test exclusion needed. UX: the full title
  bar is the universal "grab here" affordance; a grip-only zone adds discoverability cost with no
  benefit. **Verify in Bevy 0.18** that a close-button click does NOT propagate `Pressed` to the
  parent header (log both `Interaction` values; if it does propagate, fall back to Option B).
- **Cursor grab icon:** deferred to a separate feature — `planning/features/cursor_grab_icon.md`.
- **Open/close position reset:** confirmed — `OpenInventory`/`CloseInventory` flip `Visibility`,
  they do NOT respawn the panel. A dragged position therefore survives close/reopen within a scene.
  Scene change is the authoritative reset point (panel is respawned from RON `position`).

## Out of scope
- Persisting window positions across sessions or scene loads (drag state is ephemeral; scene change
  resets to RON `position`).
- Animated snap-back, magnetic edge-snapping, or window docking.
- Resize handles / resizable windows.
- A `draggable: bool` RON opt-out (add later only if a fixed panel is genuinely needed).
- Touch / multi-touch dragging and gamepad cursor dragging.
- Full window-manager focus stack (only a single "raised" z-index is in scope).

## Acceptance criteria
- Given the inventory panel is open, when the player presses the left mouse button on its title
  bar and moves the cursor, then the panel's top-left follows `cursor - grab_offset` in real time
  and does not jump on the initial click.
- Given a window is being dragged toward a screen edge, when the cursor would push any part of the
  window off-screen, then the window's `left`/`top` are clamped so the whole window stays within
  `window.size()`.
- Given both the inventory and the shop (or container) panels are open, when the player drags one,
  then only that panel moves and the other stays put (independent position state, no
  cross-contamination).
- Given a panel is being dragged, when the drag is in progress and on release, then nothing is
  pushed to `ActionQueue` and no `UiEvent`/`GameEvent` is emitted for the move.
- Given the player clicks the close button embedded in a title bar, when that click lands on the
  close `Button`, then the panel closes and a drag is NOT started.
- Given a window was dragged to a new position, when the player closes and reopens it within the
  same scene, then it reappears at the dragged position; when the scene reloads, it returns to the
  RON-authored `position`.
- The feature builds and behaves identically in a native build and a WASM build (no platform APIs).

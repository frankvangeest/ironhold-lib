# Feature: Cursor Grab Icon for Draggable Windows

_Status: Draft_
_Planned at: `a6acab8` (2026-06-22)_

## What
Change the OS/browser cursor to a grab hand (`CursorIcon::Grab` or equivalent) when the player
hovers over a draggable window title bar, and to a grabbing-fist icon while a drag is in
progress. Cursor resets to the default arrow on mouse-leave or drag release.

## Why
Polish companion to the `draggable_windows` feature. Without this, the title bar gives no visual
affordance that it is draggable — players have to discover it by trial and error. A grab cursor
is the standard affordance for draggable UI in every major OS and browser.

_Depends on: `draggable_windows` (shipped first)._

## Approach
Bevy exposes `CursorIcon` as a component on the `Window` entity (Bevy 0.15+). Setting it is a
simple `window.cursor_options.icon = CursorIcon::Grab` write in a hover/drag system. The cursor
icon is cosmetic and must not push to `ActionQueue`.

Two states:
- **Hovering title bar (not dragging):** `CursorIcon::Grab` (open hand)
- **Actively dragging:** `CursorIcon::Grabbing` (closed fist)
- **Otherwise:** `CursorIcon::Default`

Add a small `cursor_icon_system` in `Update` that reads `WindowDragState` (from the
`draggable_windows` feature) and `Changed<Interaction>` on `WindowTitleBar` entities, and sets
`Window.cursor_options.icon` accordingly. No new resources needed.

WASM note: browser cursor icons are set via CSS `cursor` property under the hood by Bevy's winit
backend — all standard `CursorIcon` variants work in major browsers. Verify in Chrome/Firefox.

## Tasks
- [ ] Add `cursor_icon_system` to `capabilities/window_drag.rs` (extend existing module).
- [ ] On `WindowTitleBar` hover (`Interaction::Hovered`): set `CursorIcon::Grab`.
- [ ] While `WindowDragState.active.is_some()`: set `CursorIcon::Grabbing`.
- [ ] On hover leave and drag release: reset to `CursorIcon::Default`.
- [ ] WASM browser test: verify cursor icon changes in Chrome and Firefox.

## Acceptance criteria
- Given the cursor enters a title bar, the cursor icon changes to a grab hand.
- Given a drag is in progress, the cursor icon shows a grabbing fist.
- Given the cursor leaves the title bar without dragging, the cursor resets to the default arrow.
- Given a drag completes (mouse release), the cursor resets to the default arrow.

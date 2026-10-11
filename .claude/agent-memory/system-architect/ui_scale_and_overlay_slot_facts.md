---
name: ui-scale-and-overlay-slot-facts
description: Bevy 0.18 UiScale mechanics, hand-converted Val::Px sites, single overlay slot, UiEvent single-variant prefix trap, no runtime UI-id component, minimize=0x0 facts (responsive_ui plan reviews 2026-10-11, rounds 1+2)
metadata:
  type: project
---

Facts verified during the responsive_ui.md plan reviews (2026-10-11, planned at e3a2ff2):

- `UiScale` is folded into the per-root `ComputedUiRenderTargetInfo.scale_factor`
  (`bevy_ui-0.18.0/src/update.rs` `propagate_ui_target_cameras`), so root `Percent(100)` spans
  `window/UiScale`. `WindowResized` is `#[derive(Message)]`.
- Code converting viewport pixels to `Val::Px` by dividing by `window.scale_factor()` alone breaks
  under UiScale != 1: `camera.rs` ~874 (split label) and ~987 (target HUD). Text2d world labels are unaffected.
- Overlays are a single global slot (`UnloadOverlay` despawns every `OverlayEntity`). Overlay loads
  do NOT emit `SceneEvent::Ready` (only full loads do, `scene_loader.rs` ~1332).
- `play.html`/`test_web.py` treat "a canvas exists" as "engine started".

Round 2 (SetUiVisible / orientation):
- **`UiEvent` has ONE variant, `ButtonPressed(String)`**, and both interpreters destructure it
  irrefutably and prefix `"ui.button_pressed:"`. "Send X as a UiEvent" with a non-button name needs a
  new variant + both interpreters (+ dialogue.rs reader) updated; otherwise it arrives as
  `ui.button_pressed:window.orientation:...`.
- **UI nodes carry no runtime id component.** `spawn_ui_element_node` (`scene_loader.rs` ~1788) only
  sets `Name` ("Group: id", "Rect: id", but Label's Name uses its *text*). Any act-on-UI-by-id
  action needs a new `UiNodeId` component inserted at every arm. UI ids are unique per scene
  (`GameSceneV2::validate`), Groups may be id-less.
- Panels (Inventory/Shop/Container) toggle `Visibility` (not `Display`) in executor arms and drive
  `panels_open`, which gates interact/pickup/tab-target. A `Display::None` ancestor silently hides an
  "open" panel while `panels_open > 0` -> input locked invisibly.
- ActionBar keyboard/gamepad slots fire regardless of the bar's visibility; two bars sharing keys
  is a `cross_bar_duplicate_key` validate ERROR. "Two HUDs swapped by visibility" breaks if both
  contain ActionBars.
- Interpreter reads UI -> Game -> Scene per frame, so a same-frame UiEvent's actions run BEFORE
  `scene.ready` rules' actions (scene.ready rules can undo it).
- `spawn_scene_v2.before(fsm_interpreter_system)` gets an auto sync point, so UI entities spawned
  with Ready exist when that frame's executor runs.
- Minimize: winit 0.30.12 Windows `WM_SIZE` always sends `Resized(lparam w,h)` = 0x0 on minimize;
  bevy_winit `react_to_resize` sets resolution 0x0 and writes `WindowResized{0,0}`. Bevy guards
  itself (render extraction skipped at 0 target, viewport `clamp_to_size`, projection update skipped
  at 0; `PerspectiveProjection::update` would `expect`-panic at 0 if called directly). Our code has no
  divide-by-window-size site (lib.rs:656 halves only; split viewport `.max(1,1)`).

Round 3:
- `UiEvent` irrefutable destructures (compile breaks on a 2nd variant): `fsm_interpreter.rs:36`,
  `entity_fsm_interpreter.rs:25`, `dialogue.rs:110` (filter_map closure pattern, not line 90),
  `tests/same_frame_order_tests.rs:75`. Other test uses are `matches!` (safe). CLI orphan check only
  inspects `ui.button_pressed:` events, so new namespaces are not flagged.
- `FsmEventBinding` = `(event: "...", do_actions: [...])`, deny_unknown_fields, no condition field.
  Plans keep writing `on:`/`do:` — always check.
- Nothing in core writes `Node.display` today (UI hiding is all `Visibility`), so `Display::None`
  has no competing writer. `panel_nested_in_group` is only a Warning: panels/ActionBars CAN sit in Groups.
- 13 `UiNodeDef` variants (Button, IconButton, Label, Rect, StatBar, StatSpread, StatRadar, ActionBar,
  4 panels, Group).

Final check (P2 split into ui_set_visible.md): when a primitive is split out as a prerequisite, grep the
*downstream* plan's earlier phases for hidden uses of it (Phase 1c "UI id rect" assertions need
`UiNodeId`) and the split plan for items whose decision depends on a later downstream spike
(`initially_hidden` hinged on the Phase 3 flash spike = reverse dependency).

**How to apply:** for UI-by-id or UiEvent-extension plans, check the variant/prefix and the id
component first; for visibility-swap layouts, check ActionBars and panels inside the swapped subtree.

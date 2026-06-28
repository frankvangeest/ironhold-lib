# Feature: Overlay modal backdrop (click-blocking)

_Status: In Progress_
_Planned at: `e24f03f` (2026-06-28)_

## What

When `LoadSceneOverlay` loads a scene, a full-screen invisible UI rect is automatically spawned beneath the overlay content. It absorbs all pointer events so base-scene buttons cannot be clicked through the overlay panel. It is tagged `OverlayEntity` and despawns automatically with `UnloadOverlay`.

## Why

Currently base-scene UI buttons remain interactive when an overlay is showing. This is a real bug for overlays where base-scene interaction is harmful (e.g. start menu loading through an options panel). The `paused` state happens to suppress those events via FSM rules, but any overlay that doesn't gate actions in RON is broken.

## Approach

### Backdrop spawning

In `spawn_scene_v2` (scene_loader.rs), when `is_overlay == true`, spawn a backdrop node before spawning the overlay scene's UI:

```rust
commands.spawn((
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    },
    BackgroundColor(Color::NONE),   // fully transparent — click-blocking only
    ZIndex::Global(100),
    OverlayEntity,
    Interaction::default(),         // makes Bevy's picking treat this node as interactive
    Button,                         // required for Interaction to be picked up by the pointer system
))
```

The overlay scene's own root UI node gets `ZIndex::Global(101)` so it renders above the backdrop.

### Z-ordering

| Layer | ZIndex::Global | Purpose |
|---|---|---|
| Base-scene UI | 0 (default) | Normal scene buttons/panels |
| Backdrop | 100 | Full-screen click absorber |
| Overlay UI root | 101 | The actual overlay content |

### No new RON fields

The backdrop is automatic — no schema change, no new `GameSceneV2` field. `UnloadOverlay` already despawns all `OverlayEntity` entities so the backdrop cleans up for free.

If a future scene needs a visible backdrop tint (e.g. darkened pause screen), the designer can add a full-screen `Rect` element to the overlay scene itself — the engine backdrop stays transparent.

### No new actions

`LoadSceneOverlay` / `UnloadOverlay` / `ToggleOverlay` are unchanged in signature and semantics. The backdrop is a silent implementation detail.

## Tasks

- [ ] In `spawn_scene_v2`, when `is_overlay == true`, spawn a transparent full-screen `Button` node tagged `OverlayEntity` with `ZIndex::Global(100)` before the overlay's UI nodes
- [ ] Set `ZIndex::Global(101)` on the overlay scene root UI node when in overlay mode
- [ ] Verify `button_system` in `lib.rs` does not emit `UiEvent` for the backdrop (it has no `UiAction` component)
- [ ] Tests: RON integration test confirming overlay blocks interaction
- [ ] Docs: `docs/20_data_formats.md` — note in `LoadSceneOverlay` section that a transparent backdrop is auto-spawned

## Open questions

- Does Bevy's picking system block pointer events for a `Button + Interaction` node that covers the screen, even without `UiAction`? (Expected: yes — `button_system` only emits events for nodes that have `UiAction`; the picking system sees `Interaction` and intercepts regardless.)
- Should `ToggleOverlay` also spawn a backdrop when opening? (Yes — it calls `LoadSceneOverlay` internally, so the path is shared.)

## Acceptance criteria

- Given an overlay is open via `LoadSceneOverlay`, when the user clicks anywhere not covered by overlay UI, the click does not reach base-scene buttons or world-space interactables.
- Given `UnloadOverlay` fires, the backdrop entity is despawned along with the rest of the overlay.
- Given a base-scene button overlapping an open overlay, when the user clicks it, no `UiEvent::ButtonPressed` is emitted for the base-scene button.

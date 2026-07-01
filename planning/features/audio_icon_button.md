# Feature: Audio icon toggle button

_Status: Ready_
_Planned at: `53643ca` (2026-07-01)_

## What

Replace the main HUD's "Toggle Mute" text button and separate "Audio: {state}" text label
(`assets/projects/3rd_person_game_demo/scenes/main.scene.ron`) with a single icon button,
positioned top-right, that shows `audioOn.png` or `audioOff.png` depending on mute state and
still fires the existing `ui.toggle_mute` action on click.

Scope: **main HUD only**. The equivalent button+label pair in `pause.scene.ron` and
`options.scene.ron` are left as text for now (not part of this feature).

## Why

The text button + separate label is two widgets doing one job, and doesn't match the compact
icon-based HUD convention the top-right corner is meant for. This also introduces a reusable
`IconButton` UI node type for future icon-swap buttons (e.g. a notifications bell, settings gear).

## Approach

**Schema — new dedicated `UiNodeDef` variant** (architect-recommended over extending `ButtonDef`,
to keep `ButtonDef` a clean text+color primitive and follow the existing convention where
specialized UI concepts get their own variant, e.g. `StatBar`, `ActionBar`):

```rust
// schema/scene_v2.rs
pub struct IconButtonDef {
    pub id: String,
    pub action: String,       // e.g. "ui.toggle_mute" — same UiAction::Trigger pipeline as Button
    pub icon_on: String,      // asset catalog texture key, e.g. "ui/audio_on"
    pub icon_off: String,     // asset catalog texture key, e.g. "ui/audio_off"
    pub bind: String,         // GameVariables key holding "true"/"false" — selects icon_off/icon_on
    pub position: (f32, f32),
    pub size: (f32, f32),
}
```

Added as `UiNodeDef::IconButton(IconButtonDef)`.

**Why a bool-shaped GameVariable instead of the existing `audio_state` string variable:**
`audio_state` holds a *display* string (`"Muted"` / `"Sound On"`) meant for the text label — binding
icon selection to that couples the icon logic to display text wording. Instead, add a second
GameVariable, `audio_muted`, holding canonical `"true"`/`"false"`, set by the *same* two existing
`state_machine.ron` rules that already set `audio_state` (no new Rust plumbing needed):

```ron
// logic/state_machine.ron — global_on, alongside the existing audio_state rules
( event: "audio.muted",   do_actions: [ SetVariable("audio_state", "Muted"),    SetVariable("audio_muted", "true")  ] ),
( event: "audio.unmuted", do_actions: [ SetVariable("audio_state", "Sound On"), SetVariable("audio_muted", "false") ] ),
```

`ToggleMute` and `SyncAudioState` already emit `audio.muted`/`audio.unmuted` — this covers both
runtime toggling and the on-load sync, purely via RON.

**Rendering (`scene_loader.rs`)** — mirrors the existing panel close-button pattern (`Button` +
`UiAction::Trigger(trigger)` + child `ImageNode`, see the `InvCloseBtn`/`close_icon_handle` code
at `scene_loader.rs:1886-1907`), reusing `asset_catalog.textures.get(key)` to resolve both icon
keys to `Handle<Image>` — no new texture-loading path.

```rust
UiNodeDef::IconButton(icon_btn) => {
    let trigger = icon_btn.action.strip_prefix("ui.").unwrap_or(&icon_btn.action).to_string();
    let icon_on = asset_catalog.textures.get(&icon_btn.icon_on).map(|p| asset_server.load(p.clone()));
    let icon_off = asset_catalog.textures.get(&icon_btn.icon_off).map(|p| asset_server.load(p.clone()));
    // spawn Button + UiAction::Trigger(trigger) + node at position/size,
    // child ImageNode defaulting to icon_off, tagged with an IconButtonBind { bind, icon_on, icon_off } component
}
```

**New system** `icon_button_sync_system` (`Update`, alongside the existing `update_dynamic_labels_system`):
reads `GameVariables[bind] == "true"` for each `IconButtonBind` entity and swaps the child
`ImageNode.image` handle between `icon_on`/`icon_off` — **only when the resolved value changed**
(change-detection discipline per `crates/ironhold_core/src/CLAUDE.md`; guard with a `bool` field on
`IconButtonBind` tracking last-applied state, not an unconditional write every frame).

**Assets** — register the two already-on-disk-but-uncatalogued textures in
`assets/projects/3rd_person_game_demo/assets.ron`:
```ron
"ui/audio_on":  "shared/ui/common-ui-icons/audioOn.png",
"ui/audio_off": "shared/ui/common-ui-icons/audioOff.png",
```

**Scene RON** (`main.scene.ron`) — remove the `hud_mute_button` Button and `audio_state_hud` Label,
add:
```ron
IconButton((
    id: "hud_audio_toggle",
    action: "ui.toggle_mute",
    icon_on: "ui/audio_on",
    icon_off: "ui/audio_off",
    bind: "audio_muted",
    position: (984.0, 16.0),   // top-right, matches existing HUD margin convention
    size: (36.0, 36.0),
)),
```

No changes to `Action`, `ActionQueue`, the interpreter, or `AudioState` — click wiring reuses the
existing `UiAction::Trigger` → `UiEvent::ButtonPressed` → `ui.button_pressed:toggle_mute` rule
already in `state_machine.ron`.

## Tasks
- [ ] Add `IconButtonDef` struct + `UiNodeDef::IconButton` variant (`schema/scene_v2.rs`)
- [ ] Add `IconButtonBind` component (bind key, icon_on/icon_off handles, last-applied bool)
- [ ] Add `IconButton` spawn arm in `scene_loader.rs` (reuse close-button pattern)
- [ ] Add `icon_button_sync_system`, register in `Update` schedule
- [ ] Register `ui/audio_on` / `ui/audio_off` in `3rd_person_game_demo/assets.ron`
- [ ] Add `SetVariable("audio_muted", ...)` to the two existing `global_on` rules in `state_machine.ron`
- [ ] Replace `hud_mute_button` + `audio_state_hud` with the new `IconButton` node in `main.scene.ron`
- [ ] Run `python tools/asset_checker/check.py`
- [ ] Tests — integration test spawning the icon button and asserting the image swaps on `ToggleMute`
- [ ] Docs — `docs/20_data_formats.md` (new `IconButtonDef` entry), `crates/ironhold_core/src/CLAUDE.md` if the sync system has non-obvious semantics

## Open questions
- None — architect consulted on schema shape (dedicated variant over `ButtonDef` extension),
  scope confirmed (main HUD only), positioning confirmed (fixed pixel, no anchor system).

## Acceptance criteria
- Given the main HUD loaded with audio unmuted, the top-right icon shows `audioOn.png`.
- Given a click on the icon button, audio mutes, the icon swaps to `audioOff.png`, and clicking
  again unmutes and swaps back — matching the prior button+label behavior exactly, minus the text.
- Given a fresh scene load with audio already muted from a prior session state, the icon shows
  `audioOff.png` immediately (via `SyncAudioState` on scene entry) — no stale/default icon frame.

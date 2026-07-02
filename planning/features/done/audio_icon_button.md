# Feature: Audio icon toggle button

_Status: Done_
_Planned at: `53643ca` (2026-07-01)_

## What

Replaced the main HUD's "Toggle Mute" text button and separate "Audio: {state}" text label
(`assets/projects/3rd_person_game_demo/scenes/main.scene.ron`) with a single icon button,
positioned top-right, that swaps between `icon_on`/`icon_off` catalog textures depending on
mute state and still fires the existing `ui.toggle_mute` action on click.

What shipped grew well beyond the original text-swap idea into a general-purpose,
fully RON-authorable icon-button node: resting/active/hover/click tint colors and an optional
drop-shadow copy, all driven by RON fields with sensible fallbacks.

Scope: **main HUD only**. The equivalent button+label pair in `pause.scene.ron` and
`options.scene.ron` are left as text (not part of this feature).

## Why

The text button + separate label is two widgets doing one job, and doesn't match the compact
icon-based HUD convention the top-right corner is meant for. This also introduces a reusable
`IconButton` UI node type for future icon-swap buttons (e.g. a notifications bell, settings gear).

## Approach (final, as shipped)

**Schema — dedicated `UiNodeDef::IconButton(IconButtonDef)` variant** (architect-recommended over
extending `ButtonDef`, keeping `ButtonDef` a clean text+color primitive):

```rust
// schema/scene_v2.rs
pub struct IconButtonDef {
    pub id: String,
    pub action: String,                              // e.g. "ui.toggle_mute"
    pub icon_on: String,                              // catalog key shown when bind == "true"
    pub icon_off: String,                             // catalog key shown otherwise
    pub bind: String,                                 // GameVariables key holding "true"/"false"
    pub position: (f32, f32),
    pub size: (f32, f32),                             // default (36.0, 36.0)
    pub absolute: bool,
    pub icon_color: Option<(f32, f32, f32, f32)>,     // resting tint while bind == "false"
    pub active_color: Option<(f32, f32, f32, f32)>,   // resting tint while bind == "true"; falls back to icon_color
    pub hover_color: Option<(f32, f32, f32, f32)>,    // falls back to icon_color
    pub click_color: Option<(f32, f32, f32, f32)>,    // falls back to icon_color
    pub shadow_offset: (f32, f32),                    // default (-2.0, 2.0); only used if shadow_color is set
    pub shadow_color: Option<(f32, f32, f32, f32)>,   // omit = no shadow spawned at all
}
```

**Gotcha discovered during play-test**: `icon_on`/`icon_off` name which state of `bind` shows
them, *not* what the artwork visually depicts. Binding `icon_on` to a same-named catalog key
(`"ui/audio_on"` → `audioOn.png`) is a trap if the asset filenames don't happen to align with
the bind semantics — in this project `bind: "audio_muted"` means `icon_on` fires when **muted**,
so it had to point at the *no-sound* artwork, not the "audio on" filename. Future designers using
this node should sanity-check which texture key goes in which slot against what the bound
variable actually represents, not just what the filenames suggest.

**Rendering (`scene_loader.rs`)** — spawns a clickable **root** entity (`Button` + `Interaction`
+ `UiAction::Trigger` + `IconButtonRoot` marker, no image of its own — just the hit-test surface)
with up to two `ImageNode` children:
1. An optional **shadow** child (spawned first → renders behind), only when `shadow_color` is set.
   Carries `IconShadowBind { key, icon_on, icon_off }` — tracks the same bound key so its
   silhouette always matches the foreground, but its color never changes (no hover/click reactivity).
2. The **foreground icon** child (spawned second → renders on top), carrying `IconButtonBind`.

This ended up as a 3-entity structure rather than the originally-planned single entity, because
supporting a layered drop-shadow requires two independently-colored image layers — the single-
entity design from the first shipped round couldn't represent that once the shadow request came in.

**`icon_button_sync_system`** (`Update`): for the foreground child, looks up the *root's*
`Interaction` via `ChildOf`/`child_of.parent()` (Interaction lives on the root, not the cosmetic
child) and picks a color — `Pressed → click_color`, `Hovered → hover_color`, else
`resting_color` (`active_color` if `bind == "true"` else `icon_color`) — then swaps
`ImageNode.image`/`.color` only on an actual change (`Handle<Image>` and `Color` both compare
cheaply by value; no extra tracked state needed). The shadow query only syncs texture, never color.

**Click firing split**: `button_system` (the pre-existing system that both fires
`UiEvent::ButtonPressed` *and* paints the generic grey/green `BackgroundColor` hover/press
feedback) would have clobbered the icon button's transparent background and tint colors. Fixed by
excluding `IconButtonRoot` from `button_system`'s query and adding a sibling
`icon_button_click_system` that fires the same event for `IconButtonRoot` entities without
touching `BackgroundColor` at all.

**RON authoring** (`assets/projects/3rd_person_game_demo/`):
- `assets.ron` — registered `"ui/audio_on"` / `"ui/audio_off"` catalog keys.
- `logic/state_machine.ron` — the two existing `audio.muted`/`audio.unmuted` `global_on` rules
  each gained a second `SetVariable("audio_muted", "true"|"false")` action alongside the existing
  `SetVariable("audio_state", ...)`, so the bool-shaped bind variable is kept in sync purely via
  RON (no new Rust plumbing) — reusing events `ToggleMute`/`SyncAudioState` already emit.
- `scenes/main.scene.ron` — replaced `hud_mute_button` (Button) + `audio_state_hud` (Label) with:
  ```ron
  IconButton((
    id: "hud_audio_toggle",
    action: "ui.toggle_mute",
    icon_on: "ui/audio_off",   // shown while muted — see the gotcha note above
    icon_off: "ui/audio_on",   // shown while unmuted
    bind: "audio_muted",
    position: (976.0, 26.0),
    size: (36.0, 36.0),
    icon_color: (0.90, 0.75, 0.40, 1.0),
    active_color: (0.75, 0.40, 0.30, 1.0),
    hover_color: (1.0, 0.90, 0.65, 1.0),
    click_color: (1.0, 0.60, 0.15, 1.0),
    shadow_offset: (-2.0, 2.0),
    shadow_color: (0.75, 0.75, 0.75, 0.55),
  )),
  ```

No changes to `Action`, `ActionQueue`, or `AudioState` — click wiring reuses the existing
`UiAction::Trigger` → `UiEvent::ButtonPressed` → `ui.button_pressed:toggle_mute` rule.

## Tasks
- [x] Add `IconButtonDef` struct + `UiNodeDef::IconButton` variant (`schema/scene_v2.rs`)
- [x] Add `IconButtonBind` / `IconShadowBind` / `IconButtonRoot` components
- [x] Add `IconButton` spawn arm in `scene_loader.rs` (root + shadow child + icon child)
- [x] Add `icon_button_sync_system`; split click-firing into `icon_button_click_system`
- [x] Register `ui/audio_on` / `ui/audio_off` in `3rd_person_game_demo/assets.ron`
- [x] Add `SetVariable("audio_muted", ...)` to the two existing `global_on` rules in `state_machine.ron`
- [x] Replace `hud_mute_button` + `audio_state_hud` with the new `IconButton` node in `main.scene.ron`
- [x] Run `python tools/asset_checker/check.py`
- [x] Docs — `docs/20_data_formats.md` `IconButton` entry (full field table + example)
- [x] Integration tests — 7 tests in `integration_tests.rs` covering `icon_button_sync_system`
      (bind-true/active-color, bind-missing/icon-color, hover/click overrides, shadow follows
      icon swap but not color) and `icon_button_click_system` (fires once on press, doesn't
      double-fire alongside the plain-Button `button_system`).

## Open questions
None remaining — architect consulted on schema shape, scope, and positioning up front; the
root/child restructure and icon_on/icon_off semantics gotcha were resolved during play-test rounds.

## Acceptance criteria
- Given the main HUD loaded with audio muted at start (project default), the icon shows the
  no-sound artwork in `active_color` (terracotta).
- Given a click, audio unmutes, the icon swaps to the sound-wave artwork in `icon_color` (brass),
  and clicking again re-mutes and swaps back.
- Hovering shows `hover_color` regardless of mute state; holding the click shows `click_color`;
  releasing reverts to the correct resting color for the current mute state.
- The optional shadow, when configured, stays offset behind the icon and always shows the same
  silhouette as the foreground, never reacting to hover/click.
- Other HUD buttons (Dance, Spawn Chest, Hitbox toggles) retain their original grey/green
  hover/press `BackgroundColor` feedback, unaffected by the icon-button-specific systems.

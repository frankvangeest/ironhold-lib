---
name: icon-button-ui-node-pattern
description: How IconButton (two-state texture-swap toggle UI node) is wired data-driven across schema, scene_loader spawn arm, and a pure-cosmetic sync system; contrasts with the UiMaterial pattern
metadata:
  type: project
---

`UiNodeDef::IconButton` (added ~2026-07-01) is the reference shape for a **simple image-based UI node** that does NOT need a custom shader (unlike [[uimaterial_ui_node_pattern]], which is for WGSL-backed nodes). Three touchpoints, no capability module, no `SceneMaterialParams` entry:

1. **`schema/scene_v2.rs`** — `IconButtonDef` struct + `UiNodeDef::IconButton` variant. Must be added to all five `UiNodeDef` helper methods (`id`/`size`/`position`/`absolute`/`align`). `align` has no field on the struct — returning `UiTextAlign::Center` hardcoded in the enum arm is accepted (matches Rect/StatBar). `#[serde(deny_unknown_fields)]`; every visual knob is a field with a `default_*` fn.
2. **`scene_loader.rs`** spawn arm (`UiNodeDef::IconButton`) — spawns `(Button, node, UiAction::Trigger(trigger))` and one `ImageNode` child carrying `IconButtonBind`. Reuses the SAME `UiAction::Trigger` + `strip_prefix("ui.")` plumbing as `ButtonDef` — no new event type. Textures resolved via `asset_catalog.textures.get(key).map(load).unwrap_or_default()`.
3. **`lib.rs`** — `IconButtonBind` component (`key`, `icon_on`, `icon_off`, `showing_on: Option<bool>`) + `icon_button_sync_system` in `Update`. System takes ONLY `Res<GameVariables>` + `Query` — no `ActionQueue`, no `Commands`. Pure cosmetic swap, change-guarded via `showing_on` (target_indicator precedent).

## Why this is fully aligned
The whole toggle loop is RON-authored: click → `UiAction::Trigger` → existing Action → executor emits semantic event → `global_on` bridge `SetVariable(bind_key, "true"/"false")` → sync system swaps texture on the bound var. Generic — works for any two-state icon toggle, not just audio mute. Designer authors icon_on/icon_off/bind/action in scene RON with zero recompile.

## button_system background collision (added ~2026-07-01 tint round)
The node collapsed from parent+child to a **single entity**: `(Button, node, BackgroundColor(Color::NONE), ImageNode{color:icon_color}, UiAction::Trigger, IconButtonBind)`. Tint feedback moved onto the icon itself via `hover_color`/`click_color: Option<(f32,f32,f32,f32)>` (multiply-tint, `ActionSlotDef.icon_color` convention), read by `icon_button_sync_system` matching on `&Interaction`. **Footgun:** the generic `button_system` (lib.rs) matches `(&mut BackgroundColor, With<Button>)` on `Changed<Interaction>` and unconditionally writes opaque grey/green backgrounds — it will overwrite the IconButton's `Color::NONE` transparent background on first hover/click, defeating the transparent-background intent and fighting the icon tint. Fix: add `Without<IconButtonBind>` to `button_system`'s query filter. Any future `Button`-based node that wants a non-default background must dodge `button_system` the same way.

## Footguns to flag on future changes
- **Missing-key silent fallback**: `unwrap_or_default()` on a missing catalog key yields a blank `Handle::default()` with NO `warn!`. Does not fabricate a path (hard rule satisfied, consistent with other file sites) but a typo'd key = silent invisible icon. Recommend adding a `warn!`. Non-blocking.
- **Initial-state seeding**: the bound GameVariable must be seeded before first interaction or the icon shows the `icon_off` default. In 3rd_person_game_demo this works because `SyncAudioState` sits in the `playing`/`menu`/`options` entry_actions and re-emits `audio.muted`/`audio.unmuted`, which the `global_on` bridge maps to `SetVariable("audio_muted", ...)`. A new IconButton on a var with no equivalent seeding action will mis-show until the first toggle. Check for a seeding path when reviewing.
- `query.rs` does NOT match on `UiNodeDef` variants, so adding a variant does NOT break the CLI exhaustive-match (unlike Action variants). No CLI touchpoint needed for new UI nodes.

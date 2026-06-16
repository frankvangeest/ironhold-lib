# Feature: Per-Prefab `depth_scale` Honoured on Dynamic Spawns

_Status: Ready_
_Planned at: `0f86e07` (2026-06-17)_

## What

`StatLabelDef.depth_scale` and `WorldStatBarDef.depth_scale` let designers override depth-based font/bar scaling on a per-prefab basis. This override is respected for scene-placed entities but is silently ignored for entities spawned at runtime via `Action::Spawn` — those always use `depth_scale: None` because the dynamic stat-UI drain system has no access to the current scene's label depth-scale config at the time of spawning.

## Why

A designer who sets `depth_scale: Some(true)` on a prefab's `stat_label` expects it to work regardless of whether that prefab is placed in the scene RON or spawned at runtime. The silent override means combat-spawned enemies (e.g. via wave spawn) display differently from scene-placed ones using identical prefab definitions, confusing both designers and players.

## Approach

Store the resolved scene-level label depth scale in a resource at scene load time so the dynamic path can read it during drain.

1. **New resource** — `LoadedLabelDepthScale(Option<(f32, f32)>)` (or reuse an existing scene-state resource if one exists). Inserted/updated by `scene_loader` when a scene finishes loading, mirroring how `ActiveTonemapping` is stored.

2. **Scene loader** — when building the `LoadedScene` state, also insert `LoadedLabelDepthScale` with the resolved `(base_font_size, depth_range)` from the active scene's stat display config (or `None` if absent).

3. **`drain_dynamic_stat_ui_system`** — read `Res<LoadedLabelDepthScale>` and propagate the value into each new `DynamicStatUiEntry` instead of hardcoding `depth_scale: None`. The per-prefab `StatLabelDef.depth_scale` field is the override; fall back to the scene-level resource if the prefab field is `None`.

No schema changes. No new RON fields. Backwards-compatible — `depth_scale: None` on both prefab and scene continues to mean "no depth scaling", same as today.

## Tasks

- [ ] Add `LoadedLabelDepthScale` resource (or identify and reuse an existing scene-state resource)
- [ ] Populate `LoadedLabelDepthScale` in `scene_loader` at scene-load completion
- [ ] Update `drain_dynamic_stat_ui_system` to read `LoadedLabelDepthScale` and apply it per entry
- [ ] Add integration test: spawn via `Action::Spawn` on a prefab with `depth_scale: Some(true)`, assert the `DynamicStatUiEntry` carries a non-`None` depth scale
- [ ] Docs: note in `docs/20_data_formats.md` that `depth_scale` on `StatLabelDef` / `WorldStatBarDef` now works for both scene-placed and dynamically spawned entities

## Open questions

- Is there a `LoadedScene` or `ActiveSceneConfig` resource that already carries this kind of per-scene state? If so, add `label_depth_scale` as a field there rather than a separate resource.

## Acceptance criteria

- A prefab with `stat_label: ( depth_scale: true, ... )` spawned via `Action::Spawn` has the same depth-scaled label behaviour as the same prefab placed directly in scene RON.
- A prefab with no explicit `depth_scale` field still behaves exactly as before (no regression).

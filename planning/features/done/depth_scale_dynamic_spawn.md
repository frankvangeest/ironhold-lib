# Feature: Dynamically-Spawned Stat Labels/Bars Inherit Scene `label_depth_scale`

_Status: Done_
_Planned at: `0f79cc8` (2026-06-17)_
_Corrected at: `de9cba8` (2026-07-10) — see Correction below._
_Shipped at: `33b7938` (2026-07-11) — dev playtest confirmed via screenshot comparison in `stats_demo` (regression check on the untouched scene-placed depth-scale path; the dynamic-spawn fix itself is covered by 3 automated tests, no live demo trigger exists for it yet)._

## Correction (2026-07-10)

The original version of this plan assumed `StatLabelDef.depth_scale` / `WorldStatBarDef.depth_scale`
were existing per-prefab RON override fields. They are not — verified against
`crates/ironhold_core/src/schema/catalog.rs`: neither struct has a `depth_scale` field, and both
carry `#[serde(deny_unknown_fields)]`, so the plan's own acceptance-criteria RON snippet
(`stat_label: (depth_scale: true, ...)`) would be a parse error today. The only schema types with a
per-label `depth_scale: Option<bool>` are `EntityLabelDef`/`WorldLabelDef` (world/portal labels),
not stat widgets.

Verified via `scene_loader.rs:1021` and `:1037`: scene-placed stat labels/bars already call
`resolve_label_depth_scale(scene.label_depth_scale.as_ref(), None)` — `per_label` is hardcoded to
`None` because there is no per-widget override to pass. Depth scaling for stat widgets is purely
**inherited from the scene's `label_depth_scale` block** (`GameSceneV2::label_depth_scale`), never
overridden per-prefab. The real (narrower) bug: `drain_dynamic_stat_ui_system` hardcodes
`depth_scale: None` for dynamic spawns instead of making that same call. This is a straight
consistency fix, no new schema surface, no new RON fields — the "no schema changes" framing in the
original plan was correct, just for a different reason than originally stated.

## What

A scene's `label_depth_scale` block makes stat labels/bars shrink with camera distance for
scene-placed entities. Entities spawned at runtime via `Action::Spawn` never get this treatment —
`drain_dynamic_stat_ui_system` has no access to the scene's `label_depth_scale` config at drain
time, so it always passes `depth_scale: None`, regardless of what the scene authored.

## Why

A wave-spawned enemy and a scene-placed enemy using the identical prefab should look the same.
Today the wave-spawned one's stat label/bar never shrinks with distance while the scene-placed one
does — a silent, undocumented inconsistency a designer would only discover by noticing it in
playtesting, since nothing in the docs says depth scaling doesn't apply to dynamic spawns.

## Approach

Store the scene's `label_depth_scale` block (not a pre-resolved tuple) in a new resource at scene
load, and call the existing `resolve_label_depth_scale` helper from the dynamic-spawn drain path —
the exact same call scene-placed stat widgets already make.

1. **New resource** — `LoadedLabelDepthScale(pub Option<crate::schema::scene_v2::LabelDepthScaleDef>)`,
   `#[derive(Resource, Default, Clone)]`. Store the def itself, not a resolved `(f32, f32)` — the def
   type already derives `Clone` and this lets the dynamic path call the same helper function
   scene-placed widgets use, rather than duplicating its precedence logic.
2. **Scene loader** (`spawn_scene_v2`) — insert `LoadedLabelDepthScale(scene.label_depth_scale.clone())`
   at the same point `ActiveTonemapping` is inserted (`scene_loader.rs:686`). Add
   `init_resource::<LoadedLabelDepthScale>()` in the plugin so a drain before any scene load yields
   `None` (today's behavior, unchanged). Clear/re-set on `Action::LoadScene` for consistency with the
   other `Active*`-style scene-state resources (redundant given the unconditional re-insert on every
   scene load, but matches convention).
3. **`drain_dynamic_stat_ui_system`** — add `res: Res<LoadedLabelDepthScale>`, and replace each
   hardcoded `depth_scale: None` for `stat_label` and `world_stat_bar` widgets with
   `resolve_label_depth_scale(res.0.as_ref(), None)` — identical to the scene-placed call sites at
   `scene_loader.rs:1021`/`:1037`. `resolve_label_depth_scale` already lives in `scene_loader.rs`, so
   no visibility change needed.

**Scope guard:** leave the four `depth_scale: None` sites in `action_executor.rs` (damage popups /
ephemeral floating text) untouched — those are intentionally not depth-scaled.

No schema changes. No new RON fields. Backwards-compatible — a scene with no `label_depth_scale`
block continues to mean "no depth scaling" for dynamic spawns, same as today.

## Tasks

- [x] Add `LoadedLabelDepthScale(Option<LabelDepthScaleDef>)` resource + `init_resource`
- [x] Populate `LoadedLabelDepthScale` in `spawn_scene_v2` (scene load). **Not** explicitly re-cleared on `Action::LoadScene` (unlike the task originally said) — deliberately dropped as redundant: `spawn_scene_v2` unconditionally re-inserts on every scene load, and `PendingEntitySpawns` is separately cleared on `Action::LoadScene`, so no spawn queued against the old scene can ever drain against a stale value. See the doc comment on `LoadedLabelDepthScale` in `mod.rs`.
- [x] Update `drain_dynamic_stat_ui_system` to call `resolve_label_depth_scale(res.0.as_ref(), None)` for both `stat_label` and `world_stat_bar` (Ascii style) widgets, replacing the hardcoded `None`. `Pixel`-style bars deliberately keep `depth_scale: None`, matching the scene-placed anchor's own pre-existing exclusion.
- [x] Integration tests (`crates/ironhold_core/tests/spawn_tests.rs`): `test_dynamic_stat_label_inherits_scene_label_depth_scale`, `test_dynamic_world_stat_bar_inherits_scene_label_depth_scale` (both push directly onto `DynamicStatUiQueue` — cover the *read* side), and `test_scene_load_populates_label_depth_scale_for_dynamically_spawned_prefab` (drives a real scene load + `Action::Spawn` — covers the *populate* side; added after `system-architect`/`debug-detective` review both flagged the first two didn't exercise `spawn_scene_v2`'s wiring).
- [x] Regression test: `test_dynamic_stat_label_has_no_depth_scale_when_scene_has_no_block` — a scene with no `label_depth_scale` block still yields `depth_scale: None` for a dynamically spawned stat widget.
- [x] Docs: `docs/20_data_formats.md`'s `label_depth_scale` scene-row updated to state it also applies to entities spawned via `Action::Spawn`, and that stat widgets have no per-widget override (unlike world labels); Pixel-bar-style exclusion cross-referenced.
- [x] Corrected the stale internal note in `crates/ironhold_core/src/CLAUDE.md` (Dynamic spawning section) that described a non-existent `depth_scale: Some(true)` per-prefab override field.
- [ ] Correct/remove the matching entry in `planning/claude_suggestions.md` once this ships

## Acceptance criteria

- A scene with a `label_depth_scale` block: a `stat_label`/`world_stat_bar` prefab spawned via
  `Action::Spawn` shrinks with camera distance identically to the same prefab placed in scene RON.
- A scene with no `label_depth_scale` block: dynamically spawned stat widgets behave exactly as
  before (no depth scaling) — no regression.
- `style: Pixel` world stat bars remain unaffected either way (pre-existing, documented exclusion).

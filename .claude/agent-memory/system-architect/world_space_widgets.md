---
name: world-space-widgets
description: Boundary between per-prefab floating widgets (stat_label/world_stat_bar) and scene-managed nameplate policy; share the bar primitive, not the systems
metadata:
  type: project
---

Three floating world-space widget systems exist/planned above 3D entities. They split into TWO categories, not three points on one spectrum:

- **Per-prefab authoring primitives** — `stat_label` (StatLabelDef) and `world_stat_bar` (WorldStatBarDef, has Ascii + Pixel styles). Authored on `PrefabDef` as `Option<...>`, single-instance, always-visible, no scene context, spawn inline at entity-spawn (and via `DynamicStatUiQueue` for `Action::Spawn`). Implemented in `capabilities/stat_display.rs`.
- **Scene-managed policy** — nameplate system (planned, `planning/features/nameplate_system.md`). Defining trait is its LIFECYCLE/POLICY: deferred spawn post-`SceneEvent::Ready`, scene-wide opt-in (`show_nameplates`), faction filter, per-frame distance culling via `NameplateSceneConfig` resource.

**Decision (advised 2026-06-23): partial consolidation, not full.** The only genuine duplication is the pixel-bar fill primitive. `WorldPixelBarFillMarker` + `world_pixel_bar_update_system` in stat_display.rs (lines ~201-261) already do left-anchored ratio→fill-width + color-band selection. The nameplate must REUSE this marker/system, not reimplement `NameplateBar`/`nameplate_update_system`.

**Why:** Full consolidation would pollute the simple case (training-dummy bar) with scene-level policy config, force the inline + DynamicStatUiQueue spawn paths to learn the deferred lifecycle, and produce one wide multi-mode struct instead of cohesive systems. "Fewer systems" is illusory.

**How to apply (boundaries to enforce):**
- `stat_label`/`world_stat_bar` NEVER read scene-level config. A field needing scene context is a signal it belongs in nameplate.
- Nameplate owns visibility policy; the fill-update system owns rendering only — it must never learn distance/faction. Visibility = `Visibility::Hidden` toggled separately.
- One pixel-bar fill primitive (`WorldPixelBarFillMarker`), one fill-update system. A 4th consumer (boss bar) reuses it.
- `StatLabelMarker` (live stat readout, updates each frame) stays distinct from nameplate static name `Text2d` (display name, set once). Looks similar, different meaning.

**Open risks flagged for the feature file:**
- Screen-pixel (world_stat_bar Pixel) vs world-unit (nameplate) `full_width` semantics both feed the same marker field — resolve coordinate-space convention before coding.
- Dynamic-spawn parity gap: nameplates spawn off SceneEvent::Ready so wave-spawned (`Action::Spawn`) entities get none. Recommend setup system poll for NameplateTag-without-anchor, not just on Ready.
- Nameplate scene-config + per-prefab `Option<bool>` override should mirror the existing `label_depth_scale` (scene) + `depth_scale: Option<bool>` (per-prefab) idiom in scene_v2.rs/catalog.rs — don't invent a new override pattern.
- Reusing ColorMaterial/Mesh2d means nameplate bars share the WASM render pipeline with world_stat_bar Pixel bars → zero new pipeline variants when a scene already uses Pixel bars.

Drift hazard: WorldStatBarDef and NameplateOptionsDef.StatBarDef describe the same concept in two structs — same class of bug as [[fragile_modules]] EffectDef/LayerDef sync. Mitigation: both construct the SAME runtime WorldPixelBarFillMarker; the single update system is the sync point.

**Nameplate shipped (reviewed ~2026-06-25) — two non-obvious facts:**
- The nameplate **anchor is a top-level (unparented) `WorldLabel` entity**, deliberately NOT a child of the tracked entity (same reasoning as `target_indicator` — avoid inheriting animation/scale transforms). Consequence: the anchor + Text2d + per-bar Mesh2d/ColorMaterial are NOT freed by `Action::Despawn` of the tracked entity; only `world_label_screen_pos_system` hides the orphan, and `LevelEntity` cleanup frees it on the next `LoadScene`. This is a real per-entity leak in wave/respawn scenes. Correct fix is a `RemovedComponents<NameplateTag>`-driven cleanup system that despawns `NameplateAnchor.0` — keep the anchor unparented, do not parent it to fix the leak.
- **Two-writer `Visibility` hazard:** both `nameplate_visibility_system` (policy: distance/faction) and `world_label_screen_pos_system` (on-/off-screen projection) write the anchor's `Visibility`. As shipped, the policy system only ever force-Hides and relies on run-order (`.after(world_label_screen_pos_system)`) + the projection system to re-Show. This is a fragile implicit contract — re-show lags one frame and breaks if a third writer or reorder appears. Preferred design: a `NameplatePolicyHidden` marker the projection system ANDs against, so the two concerns don't both write the same component.
- Spawn-condition predicate `nameplate != Some(false) && (show_nameplates || nameplate == Some(true))` was copy-pasted across 5 spawn sites — same divergence anti-pattern `tag_spawned_entity` was created to kill. Flag for extraction into one shared helper.

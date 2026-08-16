---
name: depth-scale-field-scope
description: depth_scale is a scene-label field only; StatLabelDef/WorldStatBarDef have NO depth_scale field despite docs/CLAUDE.md claiming otherwise
metadata:
  type: project
---

`depth_scale: Option<bool>` exists ONLY on the scene-level label types — `EntityLabelDef` (entity `label:`) and `WorldLabelDef` (scene `world_labels:`), both in `schema/scene_v2.rs`. It is a per-label opt-in/opt-out of the scene's `label_depth_scale` block.

`StatLabelDef` (`stat_label:`) and `WorldStatBarDef` (`world_stat_bar:`) in `schema/catalog.rs` have **NO** `depth_scale` field, and both use `#[serde(deny_unknown_fields)]` — so authoring `stat_label: (depth_scale: true)` is a hard RON parse error, not a silent no-op. Stat widgets only ever *inherit* the scene-level `label_depth_scale` (the runtime `resolve_label_depth_scale(scene.label_depth_scale, None)` call always passes `None` for the per-prefab arg on the stat branches).

**History (resolved 2026-07-11, feature/depth-scale-dynamic-spawn):** The dynamic-spawn gap was fixed — dynamically-spawned stat labels/bars now call `resolve_label_depth_scale(res.0.as_ref(), None)` identically to scene-placed ones, so both inherit `label_depth_scale`. The docs that previously (incorrectly) claimed a per-prefab `depth_scale` override field on stat widgets were corrected: `docs/20_data_formats.md` `label_depth_scale` row (~line 177) now cleanly contrasts "world labels CAN override per-label; stat widgets have NO per-widget override and always inherit"; `crates/ironhold_core/src/CLAUDE.md` "Dynamic spawning" section replaced its stale "Known limitation" note with an accurate description. Both are now schema-accurate as of that branch.

**Nameplates joined the inheritance set (feature/nameplate-zoom-spacing, ~2026-08):** `nameplate_setup_system` now calls `resolve_label_depth_scale(LoadedLabelDepthScale, None)` instead of hardcoding `None`. Nameplates therefore inherit the scene block with no per-widget override, exactly like stat widgets.

**ALL FOUR bar styles now depth-scale (same branch — verified 2026-08-16):** `stat_display.rs` previously passed `depth_scale: None` at the Pixel, Icon, and Textured anchor construction sites; it now passes `ctx.depth_scale` at all six sites. Pixel/Icon/Textured bars therefore shrink via the anchor's `Transform.scale` where they were previously fixed-size. This is a silent visual change to every shipped scene that has a `label_depth_scale` block plus one of those styles (`primitive_world` Icon hearts, `stats_demo`, `3rd_person_game_demo` Textured player bar + Pixel enemy bars). Docs line 181 was updated to "all four styles", but the per-style `size` rows were NOT: line ~4030 (`Pixel.size` "Size is constant at all camera distances") and line ~4059 (`Textured.size` "Constant at all camera distances (no depth scaling, same limitation Pixel has)") are STALE — check whether they have been corrected before quoting them.

**The formula every tuning discussion needs (lib.rs `world_label_screen_pos_system`):**
`scale = clamp(reference_distance / camera_distance, min_scale, 1.0)`.
Consequences designers are never told in docs:
- It NEVER grows a label above 1.0 — zooming *in* closer than `reference_distance` does nothing. Any "labels look oversized when zoomed in" complaint cannot be fixed by this system.
- The only band where scaling actually varies is `reference_distance` → `reference_distance / min_scale`. Outside that band it is flat. (e.g. `8.0` + `0.5` ⇒ active only between 8 m and 16 m.)
- Text2d-bearing labels (`stat_label`, Ascii bars) scale by FONT SIZE (crisp); anchor-style widgets (nameplates) scale the whole child subtree via `Transform.scale` (resampled — small text can look soft).
- Adding a `label_depth_scale` block to a scene retroactively enables scaling for EVERY world label / entity `label:` / `stat_label` / Ascii bar in that scene, not just the widget you were tuning for.

**Tuning evidence (playtest screenshots, 3rd_person_game_demo, 2026-08-16):** `reference_distance: 8.0, min_scale: 0.25` was authored for the nameplate-zoom fix. At max orbit zoom-out (radius 18, real camera-to-NPC distance ~16-26 m) nameplate NAMES render as illegible smudges and health bars become ~20 px dashes with sub-pixel numbers — the fix cured crowding but crossed into unreadable. Recommend `reference_distance: 12.0, min_scale: 0.5` as the starting point for scenes with a 3-18 orbit range: the scene has ample empty space at full zoom-out, so the crowding budget can afford larger labels.

**Round-2 retune (2026-08-16):** `3rd_person_game_demo` shipped `(8.0, 0.25)`, a playtest showed illegible text at max zoom-out, and it was retuned to `(12.0, 0.5)` with the whole history recorded in `main.scene.ron`'s comment — a good precedent to point at for "record what you tried and why".

**Separate, still-open defect the screenshots exposed (FIXED round 2 — see [[screen-offset-stacking]]):** depth scaling scales the *glyphs/subtree* but NOT the vertical row offsets within a nameplate stack (name / bar / "75 / 75" number). Those offsets are world-space, so close-in they balloon (observed ~41 px between a spider's bar and its number at max zoom-in, and labels floating far above their models into the sky/HUD band) while glyph scale is clamped at 1.0. This is the residual half of "looks wrong zoomed in" — it is NOT fixable by tuning `min_scale`/`reference_distance`.

**How to apply:** When reviewing anything about stat-widget depth scaling, still verify against the actual schema (`catalog.rs` StatLabelDef line ~945 / WorldStatBarDef line ~974) — the field genuinely does not exist and `deny_unknown_fields` makes authoring it a parse error. Pixel-style stat bars never depth-scale at all (pre-existing limitation, documented in docs/20_data_formats.md). Related: [[color_tuple_inconsistency]].

---
name: depth-scale-field-scope
description: depth_scale is a scene-label field only; StatLabelDef/WorldStatBarDef have NO depth_scale field despite docs/CLAUDE.md claiming otherwise
metadata:
  type: project
---

`depth_scale: Option<bool>` exists ONLY on the scene-level label types — `EntityLabelDef` (entity `label:`) and `WorldLabelDef` (scene `world_labels:`), both in `schema/scene_v2.rs`. It is a per-label opt-in/opt-out of the scene's `label_depth_scale` block.

`StatLabelDef` (`stat_label:`) and `WorldStatBarDef` (`world_stat_bar:`) in `schema/catalog.rs` have **NO** `depth_scale` field, and both use `#[serde(deny_unknown_fields)]` — so authoring `stat_label: (depth_scale: true)` is a hard RON parse error, not a silent no-op. Stat widgets only ever *inherit* the scene-level `label_depth_scale` (the runtime `resolve_label_depth_scale(scene.label_depth_scale, None)` call always passes `None` for the per-prefab arg on the stat branches).

**History (resolved 2026-07-11, feature/depth-scale-dynamic-spawn):** The dynamic-spawn gap was fixed — dynamically-spawned stat labels/bars now call `resolve_label_depth_scale(res.0.as_ref(), None)` identically to scene-placed ones, so both inherit `label_depth_scale`. The docs that previously (incorrectly) claimed a per-prefab `depth_scale` override field on stat widgets were corrected: `docs/20_data_formats.md` `label_depth_scale` row (~line 177) now cleanly contrasts "world labels CAN override per-label; stat widgets have NO per-widget override and always inherit"; `crates/ironhold_core/src/CLAUDE.md` "Dynamic spawning" section replaced its stale "Known limitation" note with an accurate description. Both are now schema-accurate as of that branch.

**How to apply:** When reviewing anything about stat-widget depth scaling, still verify against the actual schema (`catalog.rs` StatLabelDef line ~945 / WorldStatBarDef line ~974) — the field genuinely does not exist and `deny_unknown_fields` makes authoring it a parse error. Pixel-style stat bars never depth-scale at all (pre-existing limitation, documented in docs/20_data_formats.md). Related: [[color_tuple_inconsistency]].

---
name: decals-map-two-consumers
description: assets.ron `decals:` map is consumed by BOTH Action::ProjectDecal and scene target_indicator, documented in separate doc sections
type: project
---

The `decals:` map in `assets.ron` has two independent designer-facing consumers:
1. `Action::ProjectDecal` (from rules.ron / behavior files) — documented in docs/20_data_formats.md AssetCatalog section (~line 985, "Ground decals").
2. `GameSceneV2.target_indicator` (TargetIndicatorDef) — documented in docs/20_data_formats.md GameSceneV2 section (~line 361, "Target indicator").

**Why:** The two doc sections were written independently and (as of 2026-06) do not cross-reference each other. The AssetCatalog blurb frames `decals:` as exclusively for ProjectDecal, which can mislead a designer into thinking target_indicator needs a different map.

**How to apply:** When reviewing any new feature that consumes a decal texture, check it resolves against `decals:` (not `textures:`) and that the relevant doc section cross-links the other consumer. Watch for the `texture:`-field-named-but-resolves-against-`decals:` footgun (field name fights the map it reads from). Canonical target-ring example: 3rd_person_game_demo (assets.ron `target_ring` -> ring_thick.png; main.scene.ron target_indicator block at end).

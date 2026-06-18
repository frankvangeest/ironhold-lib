---
name: target-indicator-color-tiers
description: Target indicator ring color resolves via 3 tiers; silent fallthrough is the footgun; indicator_color has no shipped example
type: project
---

`TargetIndicatorDef.named_colors` (scene, RGBA 4-tuple map) + `PrefabDef.indicator_color` / `indicator_category` (both optional) drive selected-target ring color. Precedence (highest first): `indicator_color` > `indicator_category` (looked up in scene `named_colors`) > scene-level `color` fallback. Documented well in docs/20_data_formats.md ~lines 361-421 and PrefabDef rows 1173-1174.

**Why:** The dominant designer trap is SILENT FALLTHROUGH in both directions — a `named_colors` key with no prefab referencing it is simply unused; a prefab `indicator_category` with no matching key (incl. case mismatch, keys are case-sensitive) silently falls through to `color` with no error/warning. As of 2026-06 the docs do not state this. Also: docs example lists an `"ally"` category that exists in NO shipped prefab, and `indicator_color` (the direct-override tier) has NO worked example in any project — only `indicator_category` is demonstrated (3rd_person_game_demo: enemy_zombie/orc/snake/spider -> "enemy", creature_alpaking -> "creature"; scene named_colors only defines enemy+creature).

**How to apply:** When reviewing target-indicator color changes, check (1) the silent-fallthrough behavior is documented, (2) any doc `named_colors` key has a real prefab using it OR is trimmed, (3) `indicator_color` gains a copyable example. Reuses the `texture:`-resolves-against-`decals:` footgun — see [[decals-map-two-consumers]]. RGBA 4-tuple is used consistently here; guard against a future example slipping to 3-tuple — see [[color-tuple-inconsistency]].

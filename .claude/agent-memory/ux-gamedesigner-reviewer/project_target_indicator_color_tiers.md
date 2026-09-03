---
name: target-indicator-color-tiers
description: Target indicator ring color resolves via 3 tiers; silent fallthrough is now documented in docs/20 (~506); shipped category values are "hostile"/"neutral"/"friendly" (drifted from the earlier "enemy"/"creature"/"ally" naming)
type: project
---

`TargetIndicatorDef.named_colors` (scene, RGBA 4-tuple map) + `PrefabDef.indicator_color` /
`indicator_category` (both optional) drive selected-target ring color. Precedence (highest first):
`indicator_color` > `indicator_category` (looked up in scene `named_colors`) > scene-level `color`
fallback. Documented in docs/20_data_formats.md's TargetIndicatorDef field table and the "Ring
colour resolution" list right after it.

**RESOLVED — category values and fallthrough documentation both updated:**
- Shipped `indicator_category` values are now `"hostile"`/`"neutral"`/`"friendly"` (verified in
  `3rd_person_game_demo/prefabs/prefabs.ron`: zombie/orc/spider → `"hostile"`, snake/alpaking/chest
  → `"neutral"`, merchant/select-target dummy → `"friendly"`), matching the scene's `named_colors`
  map 1:1 (`3rd_person_game_demo/scenes/main.scene.ron`: `hostile` red, `neutral` yellow, `friendly`
  green). The old `"enemy"`/`"creature"`/`"ally"` naming this memory previously cited is stale —
  don't cite it.
- **Silent fallthrough is now documented** (docs/20 ~506, immediately after the resolution list):
  "if a prefab's `indicator_category` key is not present in `named_colors` (including a typo or
  case mismatch), the ring silently falls back to the scene-level `color`. There is no error at
  load time." Do not re-flag this as an undocumented footgun.

**Still worth checking on review:** `indicator_color` (the direct-override tier) still has no
shipped worked example in any project — only `indicator_category` is demonstrated. Verify this is
still true before citing it, since it's the kind of gap that gets closed by any random future demo.

**How to apply:** When reviewing target-indicator color changes, check any doc `named_colors` key
has a real prefab using it OR is trimmed, and push for `indicator_color` to gain a copyable example
if reviewing that area. Reuses the `texture:`-resolves-against-`decals:` footgun pattern — see
[[decals-map-two-consumers]]. RGBA 4-tuple is used consistently here.

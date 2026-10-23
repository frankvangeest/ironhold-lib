---
name: project-ambient-light-contract
description: Scene ambient light became real in prerelease_render_fixes (2026-10-13); colour is linearised, two different "default" numbers (80 vs 150), most shipped scenes untuned
metadata:
  type: project
---

Until feature/prerelease_render_fixes, `lighting.ambient`/`ambient_brightness` did nothing (every scene rendered Bevy white/80).
Now applied as GlobalAmbientLight on every base-scene load (resets when unauthored; overlay `lighting:` ignored).

Designer traps to re-check on any lighting doc/RON change:
- Colour is linearised: effective strength ~= brightness x linear luminance. sRGB 0.5 -> ~0.21, 0.25 -> ~0.05.
  So `(0.2,0.24,0.38)` at 90 is ~6% of the default. Rule: brightness needed ~= 80 / linear luminance.
- Two defaults: omit both = white/80; set only `ambient` = brightness 150 (so `ambient: (1,1,1)` alone is ~2x default).
- Only foliage_demo, custom_materials, blank_project were re-tuned; quick_scene (~4%), 3rd_person main (~5%),
  terrain_demo (~6%), local_coop rooms (~12%) still carry dark tints tuned blind. These are copy-from examples.
- Overlay-ignored and reset-per-load rules lived only in the schema doc comment, not docs/20 table (2026-10-13).
- lighting_validate (2026-10-23): `invalid_lighting` validate error + runtime warn. ONE bad ambient field drops BOTH
  to white/80 (not 150, colour not kept); shadow sizes 64..8192 pow2 fall back 2048/1024 independently.
  ambient_problem reports only the first bad ambient field per run. Check docs say "both" on future edits.

**Why:** values were authored while the field was dead, so shipped numbers are not evidence of good tuning.
**How to apply:** when reviewing lighting, compute effective strength vs 80 rather than trusting the number; see [[project-docs-lag-actions]].

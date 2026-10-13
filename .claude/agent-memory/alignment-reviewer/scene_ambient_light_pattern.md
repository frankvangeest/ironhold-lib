---
name: scene-ambient-light-pattern
description: Scene lighting.ambient was a dead AmbientLight entity until prerelease_render_fixes phase A (GlobalAmbientLight resource); sRGB-linearised colour x brightness math shows most authored scenes get far darker; scene-scoped lighting resources leak
metadata:
  type: project
---

Until `prerelease_render_fixes` phase A (plan 5217c95, 2026-10-12), `apply_lighting_v2` spawned an `AmbientLight` entity,
which in Bevy 0.18 is a per-camera override, so every scene rendered with Bevy's GlobalAmbientLight default (white, 80).
37 scenes author `ambient:`; the fix makes them real.

**Effective-ambient math to reuse when reviewing lighting changes:** the colour goes through `Color::srgba`, so a typical
authored (0.2, 0.25, 0.35) linearises to ~(0.03, 0.05, 0.10); times brightness 100-260 that is ~5-25 vs the old 80 per
channel. Nearly every authored scene gets 4-20x LESS ambient, not more. Outliers at plan time: foliage_demo (1.2) and
custom_materials (15) ~ zero ambient; particles2 (0.5,0.55,0.78)x360 and integration_tests terrain_test (0.8,0.85,1.0)x150
roughly equal/brighter and bluer.

**Scene-scoped resource leak class:** `shadow_map_size` / `point_shadow_map_size` insert resources only when `Some` and
never reset, so they leak into the next scene (pre-existing). Any new scene-scoped resource must reset on Replace when
unauthored; GlobalAmbientLight follows that after phase A.

**Unit wording:** Bevy ambient brightness is cd/m^2 (a colour multiplier), not lux; docs/20 and scene_v2.rs said "lux".

**How to apply:** any future lighting/day-night/EnvState change touches the same resource; check reset-on-Replace,
`ambient_brightness`-without-`ambient` handling, and re-baseline scope. Related: [[web-host-page-contract]].

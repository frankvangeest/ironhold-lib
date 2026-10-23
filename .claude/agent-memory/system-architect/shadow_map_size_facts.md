---
name: shadow-map-size-facts
description: Bevy 0.18 / wgpu 27 facts on DirectionalLightShadowMap / PointLightShadowMap sizes - what Bevy corrects, what crashes, real memory cost, WebGL2 vs WebGPU limits
metadata:
  type: project
---

Verified 2026-10-23 (lighting_validate review, bevy_light/bevy_pbr 0.18.0, wgpu-types 27.0.1).

- **Directional is self-healing in Bevy**: `validate_shadow_map_size` (bevy_light directional_light.rs) rewrites a
  non-power-of-two size to `next_power_of_two()` with a warn (0 -> 1, i.e. a 1x1 map, no crash), and
  `prepare_lights` (bevy_pbr render/light.rs ~1076) clamps width/height to `max_texture_dimension_2d`. So a bad
  directional size never fails texture creation; it only degrades quality.
- **Point is NOT protected**: no pow2 fixup, no clamp. Size 0 or > device `max_texture_dimension_2d` = wgpu
  validation error (native default handler panics). Non-pow2 point is technically fine (no cascade stability issue).
- **Limits Bevy uses**: priority `Functionality` (default) takes `adapter.limits()`, i.e. the real device limit.
  WebGPU spec guarantees 8192; **WebGL2 only guarantees 2048** (MAX_TEXTURE_SIZE / MAX_CUBE_MAP_TEXTURE_SIZE), and
  `Limits::downlevel_webgl2_defaults` is 2048. "WebGL2 guarantees 8192" is a wrong claim.
- **Memory** (Depth32Float, 4 B/texel): 8192^2 = 256 MiB per layer. Directional uses one layer per cascade (default 4
  on WebGPU/native, 1 on WebGL2) + spot lights -> 1 GiB at 8192. Point = 6 faces per shadow-casting light -> 1.5 GiB
  at 8192, 384 MiB at 4096, 96 MiB at 2048. 8192 is not WASM-sane; cap at 4096 (point arguably 2048).

**How to apply:** when reviewing anything that writes these resources, the crash risk is the point map only; a
runtime clamp against `RenderDevice::limits()` is the only full fix for low-limit mobile devices. Related:
[[wasm-pitfalls]].

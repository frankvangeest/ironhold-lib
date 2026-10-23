---
name: lighting-bevy-asserts-and-clamps
description: Bevy 0.18 lighting facts behind scene `lighting:` validation - cascade builder assert!s panic on authored values, directional shadow size is clamped by Bevy but point size is not, WebGL2 guarantees 2048 not 8192
metadata:
  type: project
---

Verified from Bevy 0.18 / wgpu-types 27 sources during the lighting_validate review (2026-10-23, commit 5c60af8):

- `CascadeShadowConfigBuilder::build()` (bevy_light cascade.rs:95-120) has hard `assert!`s: num_cascades > 0,
  maximum_distance > minimum_distance (0.1), overlap_proportion in [0,1). scene_loader feeds `directional.num_cascades`,
  `shadow_distance`, `cascade_overlap` straight in, so `num_cascades: Some(0)`, `cascade_overlap: Some(1.0)`, `shadow_distance: Some(0.05)` or NaN
  panic at scene load. No CLI check existed at that commit (the new `invalid_lighting` only covers ambient + shadow sizes).
- bevy_pbr render/light.rs clamps the DIRECTIONAL shadow texture to `max_texture_dimension_2d`, but NOT the point-light
  cube texture. So "too big fails at texture creation" is only true for point maps (and for size 0 in both).
  Non-power-of-two sizes create fine; power-of-two is a convention, not a GPU requirement.
- Bevy's default `WgpuSettingsPriority::Functionality` uses `adapter.limits()`; `downlevel_webgl2_defaults` has
  max_texture_dimension_2d = 2048 (WebGL2 spec minimum). 8192 is WebGPU's guaranteed default, not WebGL2's.
- Memory at 8192: Depth32Float, 4 cascades (non-webgl) = 1 GiB directional; a shadowed point light = 6 faces x 256 MiB.

**How to apply:** when reviewing any lighting/shadow validation, check these asserts and the clamp asymmetry before
accepting "fails at texture creation" rationale. Related: [[nan-from-unclamped-ron-numerics]], [[f32-clamp-panics-on-authored-bounds]].

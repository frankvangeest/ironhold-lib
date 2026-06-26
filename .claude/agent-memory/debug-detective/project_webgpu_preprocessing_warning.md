---
name: webgpu-preprocessing-warning
description: Bevy 0.18 "Some GPU preprocessing are limited" / "build mesh uniforms pipeline wasn't ready" — what they actually mean (not CPU fallback, not per-frame)
metadata:
  type: project
---

In Bevy 0.18 WebGPU builds two console messages routinely appear and are commonly misread as per-frame CPU cost. They are NOT the cause of per-frame stutter:

- `INFO Some GPU preprocessing are limited on this device.` — emitted at `bevy_render-0.18.0/src/batching/gpu_preprocessing.rs:1141`. Maps to `GpuPreprocessingMode::PreprocessingOnly`: GPU mesh-uniform preprocessing **still runs on the GPU**; only GPU *occlusion culling* is disabled. The true CPU-fallback mode (`GpuPreprocessingMode::None`) is only chosen for `wgpu::Backend::Gl` (i.e. WebGL2) or zero compute support. So WebGPU does MORE on-GPU preprocessing than the old WebGL2 path, not less.
- `WARN bevy_pbr/src/render/gpu_preprocess.rs:842 The build mesh uniforms pipeline wasn't ready` — line 842 is `warn_once!`, so it fires **once at startup**, never per frame. Cannot be a recurring-stutter cause.

**Why:** During a stutter investigation these were flagged as suspected per-frame CPU overhead. Reading the registry source falsified that: empty AdapterInfo from a browser WebGPU adapter is normal and only loses occlusion culling, which is irrelevant for a ~12-mesh scene.

**How to apply:** When a WASM stutter report blames WebGPU preprocessing, don't accept it — the per-frame regression is almost always in project per-frame systems (unconditional `Transform`/`Visibility`/`TextFont` writes that trip change detection and re-propagate to children). See [[per-frame-changedetection-transform-writes]].

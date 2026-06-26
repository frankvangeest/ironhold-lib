# Memory Index

- [WebGPU preprocessing warnings are red herrings](project_webgpu_preprocessing_warning.md) — Bevy 0.18 "preprocessing are limited" = PreprocessingOnly (still on GPU), "pipeline wasn't ready" is warn_once at startup; neither is per-frame CPU cost
- [Per-frame Transform writes dirty nameplate subtrees](project_changedetection_transform_writes.md) — world_label_screen_pos_system writes Transform.translation unconditionally every frame, re-propagating to all Text2d/Mesh2d children; guard it like the font/visibility writes

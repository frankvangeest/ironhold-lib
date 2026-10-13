---
name: ambient-light-entity-noop
description: scene lighting.ambient is a no-op in Bevy 0.18 — AmbientLight is a per-camera override with #[require(Camera)]; spawning it standalone makes a stray render-graph-less Camera
metadata:
  type: project
---

Verified from registry source 2026-10-13 (be39d84): `scene_loader.rs` (~L2998-3007) spawns `AmbientLight` on its own entity.
In Bevy 0.18 `AmbientLight` has `#[require(Camera)]` (bevy_light ambient_light.rs:9-12) and `prepare_lights` only reads it
from `With<Camera3d>` views (bevy_pbr render/light.rs:719-723), else `Res<GlobalAmbientLight>` (default white/80). ironhold
never writes `GlobalAmbientLight`, so authored ambient does nothing in all 37 scenes, and the entity becomes a bare `Camera`
that trips bevy_render's "no render graph" warn. No ironhold camera query sees it (all filter `With<Camera3d>`/markers).

**Why:** docs/20 documents ambient as working; a "my ambient tweak does nothing" report is this, not a colour/HDR issue.
**How to apply:** fix = write `GlobalAmbientLight` (or insert `AmbientLight` on the Camera3d). Not browser-confirmed yet.
Related: [[warmup-strips-all-nfc]].

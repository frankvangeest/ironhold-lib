---
name: warmup-strips-all-nfc
description: pipeline_warmup_system removes NoFrustumCulling from EVERY entity at countdown end, including particle pool meshes that need it permanently; also overlay scene.loaded timing note
metadata:
  type: project
---

`lib.rs` `pipeline_warmup_system` (~L479-502) removes `NoFrustumCulling` from all `With<NoFrustumCulling>` entities when
`PipelineWarmup` hits 0 — no marker of which it inserted. `particle_renderer.rs` (~L364/392) spawns pool-group meshes with
permanent NFC (Transform::default + world-space verts). A pool group created inside the 4-frame window (docs' scene-load
`SpawnEffect` warmup recipe) loses NFC → can be culled against a stale AABB. Code-inferred 2026-10-13, not browser-reproduced.

Same review: `spawn_scene_v2` writes `SceneEvent::Loaded` (~L87) in the same run as `Ready` (~L1333), for overlays too, and runs
`.before(fsm_interpreter_system)`, so `scene.loaded:<overlay>` likely works as a start-hidden hook for `SetUiVisible` even though
docs say "entities not yet spawned" at scene.loaded. Untested — write one test before relying on it. See [[overlay-no-scene-ready]].

**Why:** "particles vanish when the camera turns, only sometimes after a scene load" → check this first.
**How to apply:** fix with a `WarmupNoCull` marker; regression test must assert a pre-existing NFC survives warmup end.

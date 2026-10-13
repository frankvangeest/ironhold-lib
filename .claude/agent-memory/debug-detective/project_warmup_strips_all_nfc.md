---
name: warmup-strips-all-nfc
description: pipeline_warmup_system NFC history (strip-all bug fixed via WarmupNoCull on feature/prerelease_render_fixes) + Bevy calculate_bounds skips NFC entities, so a mesh mutated during warmup keeps a stale Aabb
metadata:
  type: project
---

Original bug: `lib.rs` `pipeline_warmup_system` removed `NoFrustumCulling` from all `With<NoFrustumCulling>` entities at
countdown end, stripping the permanent NFC on `particle_renderer.rs` pool-group meshes. Fixed on
`feature/prerelease_render_fixes` (2026-10-13): insert `(NoFrustumCulling, WarmupNoCull)` only on `Without<NoFrustumCulling>`
meshes, strip only `With<WarmupNoCull>`. Residual rule: a permanent NFC must be in the SAME spawn bundle as `Mesh3d`
(inserting it later onto an already-marked mesh still gets stripped).

**Still-open, pre-existing (verified in bevy_camera-0.18.0 visibility/mod.rs:451-476):** `calculate_bounds`'s
`update_aabb` query is `Or<(AssetChanged<Mesh3d>, Changed<Mesh3d>)>, Without<NoFrustumCulling>`. A mesh whose *asset is
mutated* during the 4-frame warmup window gets no Aabb update (it has temporary NFC), and the change tick is gone by the
time NFC is removed -> culled against a stale Aabb afterwards. `new_aabb` (Without<Aabb>) is fine: it retries after removal.

Same earlier review: `spawn_scene_v2` writes `SceneEvent::Loaded` in the same run as `Ready`, for overlays too. See [[overlay-no-scene-ready]].

**Why:** "something vanishes when the camera turns, only after a scene load" -> check warmup x Aabb first.
**How to apply:** procedurally-rebuilt meshes created at scene load (terrain/foliage/ocean regen) should use an explicit
`Aabb` + `NoAutoAabb`, or force an Aabb recompute after warmup.

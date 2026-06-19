---
name: camera-shake-pattern
description: CameraShake Action targets the OrbitCamera singleton via a query in SceneStateParams, not a spawn-ID; shake decay lives in a capability system that only mutates Transform (correct — no ActionQueue push)
metadata:
  type: project
---

`Action::CameraShake { duration_secs, intensity }` is the canonical example of an Action that targets an **engine-managed singleton entity** (the orbit camera) rather than a designer-named spawn-ID entity.

**Why:** Most entity-targeted actions (ShowDamagePopup, PlayAnimationOn, ResetToSpawn) resolve `entity: "{self}"` through `SpawnRegistry`. CameraShake instead iterates `scene_state.orbit_cameras: Query<Entity, With<OrbitCamera>>` (added to `SceneStateParams` in scene_manager/mod.rs) and inserts a `CameraShakeState` component. Designers do NOT name the camera — they just say "shake".

**How to apply** — checklist for camera-targeted / singleton-targeted actions:
1. Struct variant in `schema/actions.rs` with doc comment + RON example (named-field syntax: `CameraShake(duration_secs: 0.4, intensity: 0.15)`).
2. Match arm in `action_executor.rs` inserts a state component on the queried entity; warns + no-ops if the query is empty (flycam scenes have no OrbitCamera — correct graceful degradation).
3. Add the `Query` to `SceneStateParams` in `runtime/scene_manager/mod.rs` (watch the 16-param SystemParam limit — that's why SceneStateParams exists).
4. Capability system (`camera_shake_system` in capabilities/camera.rs) reads `Time`, decays, mutates ONLY `Transform`, removes the component at remaining<=0. It must NOT push to ActionQueue or emit messages — it's a pure visual side-effect of the state component, same shape as target_indicator_system.
5. Register the system in lib.rs Update, chained AFTER `camera_orbit_system` so the shake offset is applied after the orbital position is set (order matters — orbit overwrites translation each frame).
6. Doc in `docs/20_data_formats.md` + integration test.

**Determinism note:** shake uses `(t*37.0).sin()` / `(t*53.0).sin()` — deterministic, no RNG, WASM-safe. Good pattern; flag any camera-effect that reaches for `rand`.

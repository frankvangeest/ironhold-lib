---
name: camera-shake-system
description: camera_shake_system in capabilities/camera.rs — Update-chained, query-gated by CameraShakeState, idle-cheap, deterministic sine (WASM-safe)
metadata:
  type: project
---

`camera_shake_system` (crates/ironhold_core/src/capabilities/camera.rs) applies a procedural sine-wave camera shake. Registered in lib.rs Update `.chain()` immediately after `camera_orbit_system`, before `fly_camera_system`.

Query is `Query<(Entity, &mut Transform, &mut CameraShakeState), With<OrbitCamera>>` — gated by the optional `CameraShakeState` component which is inserted by `Action::CameraShake` and self-removed when `remaining <= 0.0`.

**Why it's cheap on idle frames:** archetype-filtered query. When no camera has `CameraShakeState`, the loop iterates zero entities and `&mut Transform` is never dereferenced, so Bevy change-detection does NOT fire and no transform propagation is triggered. The `Commands` param is the only unconditional cost (negligible). This is the correct pattern — do not suggest a `Local<bool>` guard or run condition; the component filter already does the gating.

**Why it's WASM-safe:** `.sin()`/`.sqrt()` are WebAssembly intrinsics (f32.sqrt is a single opcode; sin compiles to a libm call but is sub-microsecond). No RNG (deterministic), no allocations, no new deps, zero binary-size impact.

**Latent non-perf concern (out of scope but noted):** runs in Update, but `camera_orbit_system` sets `cam_transform.translation` absolutely in the same chain immediately before — shake adds on top, correct. However camera-follow living in Update rather than FixedUpdate is the project's existing choice for the visual/animation pipeline (see lib.rs comment line 229), not a regression introduced here.

Related: [[project_target_indicator_system]] (same Update-chained, component-gated, idle-cheap pattern), [[project_wasm_size]].

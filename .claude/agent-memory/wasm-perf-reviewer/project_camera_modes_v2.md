---
name: camera-modes-v2
description: camera_blend_system is archetype-empty on idle frames (free); SetCameraMode has no same-mode guard (8 archetype moves/invocation); standalone flycam lacks AuthoredCameraMode so all_cameras never matches it
metadata:
  type: project
---

Builds on [[camera-modes-v1]]. Adds `Action::SetCameraMode`, a scene-level `camera_modes:` registry
(`LoadedCameraModes` resource, Replace-branch only), `CameraTransition`/`EaseKind` schema, and three
new camera components: `AuthoredCameraMode(CameraModeDef)`, `CameraModeOverride` (zero-sized),
`CameraBlendState`.

**`camera_blend_system` (capabilities/camera.rs, last entry in lib.rs's Update camera `.chain()`)
costs nothing on idle frames.** Its `&mut CameraBlendState` term is non-optional, so `QueryState`
only matches archetypes containing it; with no blend in flight the matched-archetype list is empty
and `iter()` returns immediately. `Option<&mut Projection>` does NOT widen the archetype match.
Zero allocations — no Vec/String/`format!`; `Commands` is only touched on the final frame of a
blend. Because nothing is iterated, no camera gets a spurious `Changed<Transform>` (so no
GlobalTransform propagation churn). It MUST stay last in the chain — it re-lerps `Transform` after
every per-mode system has written its live target.

**The real cost of this feature is per-switch, not per-frame:** `entity_spawner::apply_camera_mode`
issues 6 separate `remove::<XCameraMode>()` + an insert of `ActiveCameraMode`+marker + `insert_fov`
(a `Projection` insert) + a `CameraBlendState` insert/remove — up to ~8 archetype moves on a camera
entity per invocation. **There is no "already in this mode" guard**, so a `SetCameraMode` fired
repeatedly (a `stat.changed`-style trigger on a per-frame-regen stat, or an FSM re-entering a state)
both restarts the blend from the already-partially-blended pose every frame (camera never converges)
and pays those archetype moves per frame. All demo bindings today are edge-triggered
(`ui.button_pressed` from `scene_key_bindings`), so it is not a live regression — but the guard is
the cheap defense if this ever regresses.

**Reachability gap — fixed.** The standalone flycam-tagged camera spawned in `scene_loader.rs`
(~L871-877) now inserts `AuthoredCameraMode` alongside `ActiveCameraMode`, matching every other
player-camera spawn path (orbit/split/party/follow/first-person/fixed). `SceneStateParams
::all_cameras` requires `&AuthoredCameraMode`, so before this fix it never matched the flycam and
`SetCameraMode` with `owner_player` omitted in a flycam-only scene silently resolved to an empty
target `Vec` with no `warn!`. Verified current (2026-09-03): the flycam spawn's inline comment at
that call site explicitly cites this finding as the reason for the insert.

`SceneStateParams::transforms` was narrowed to `Without<ActiveCameraMode>` to avoid a B0001 conflict
with the new `all_cameras` read of `&Transform`. Its only consumer is `Action::ResetToSpawn` (NPCs
only), so the narrowing is safe — but any future action that wants to move a camera via that query
will silently `Err` out of `get_mut`.

`dynamic_split_screen_system`'s new `Has<CameraModeOverride>` term is archetype-level and adds zero
per-entity work.

Zero new dependencies (no `Cargo.toml`/`Cargo.lock` diff). No `std::thread`/`std::fs`/blocking I/O;
`Quat::slerp`/`Vec3::lerp`/`f32::powi` are glam-scalar-or-SIMD and compile on wasm32; nothing here
touches wgpu features, shaders, or uniform/storage structs. See [[project-wasm-size]].

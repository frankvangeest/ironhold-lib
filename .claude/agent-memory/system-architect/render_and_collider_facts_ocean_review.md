---
name: render-and-collider-facts-ocean-review
description: Verified Bevy 0.18 / engine facts from the ocean_simulation_demo plan-review (2026-10-02) - AmbientLight requires Camera, warmup strips ALL NoFrustumCulling, NoAutoAabb, WGSL sin range, Assets::iter_mut marks Modified, PrefabDef.colliders is GLB-only
metadata:
  type: project
---

Facts verified against vendored source / code at `34803b1` (2026-10-02) while plan-reviewing
`planning/features/ocean_simulation_demo.md`. Re-verify line numbers before citing.

- **`AmbientLight` is a per-camera override component in Bevy 0.18** (`bevy_light-0.18.0/src/ambient_light.rs`,
  `#[require(Camera)]`); the scene-wide value is the `GlobalAmbientLight` resource. `scene_loader.rs` (~2882)
  spawns `AmbientLight` on a standalone entity, so `lighting.ambient` most likely creates a stray bare `Camera`
  and never changes the real Camera3d's ambient. Unlogged latent bug as of this review. Any lighting/env feature
  must write `GlobalAmbientLight` (or the Camera3d's own `AmbientLight`).
- **`pipeline_warmup_system` (lib.rs ~446) removes `NoFrustumCulling` from EVERY entity that has it** when the
  4-frame countdown ends, not just ones it added. "Permanent NFC" on any scene-load-time entity is silently undone.
  For displaced meshes use an inflated explicit `Aabb` + `NoAutoAabb` (bevy_camera visibility/mod.rs:445-469;
  `calculate_bounds` skips `NoAutoAabb`).
- **WGSL `sin`/`cos` accuracy is only specified inside [-pi, pi]** - shaders evaluating world-space phase must
  range-reduce (fract) and the CPU mirror must use the identical reduction for parity.
- **`Assets::iter_mut` / `get_mut` queue `AssetEvent::Modified`** for every visited asset -> material re-prepare.
  `foliage_lighting_sync_system` iter_muts all FoliageMaterials every frame (pre-existing small perf smell).
- **`PrefabDef.colliders` is consumed only by `spawn_prefab_instance` (GLB Actor/Prop)**; Primitive prefabs get
  collision only from `primitive.physics` / `children[].primitive.physics` (parent gets `RigidBody::Fixed`).
  `attach_prefab_features` runs after those inserts on all 3 paths -> the right place for a later
  `RigidBody::Dynamic` override. Composite-NPC precedent already overrides Fixed with Dynamic this way.
- `AdditionalMassProperties::Mass` scales collider-derived inertia but cannot move COM; use `MassProperties`
  variant + `ReadMassProperties` (bevy_rapier 0.33) to get COM for `ExternalForce::at_point`.
- bevy_rapier 0.33 `ExternalForce` sync (`plugin/systems/rigid_body.rs` ~312) runs on `Changed<ExternalForce>`:
  reset_forces + add_force - persistent until the component changes again.

**Why:** these were each load-bearing for a plan claim that was wrong. **How to apply:** check them in any
lighting, custom-vertex-material, buoyancy/physics-body, or boat/vehicle review. Related: [[player-marker-gap]]
(boat-as-player blocked by CharacterController-as-player-marker), [[shader-resolution-pattern]].

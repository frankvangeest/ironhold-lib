---
name: target-indicator-system
description: target_indicator_system per-frame cost, Local cache lifecycle, and Blend/depth_bias parity with decal capability
metadata:
  type: project
---

`target_indicator_system` (capabilities/target_indicator.rs) runs in `Update` (NOT in the chained interpreter set). Cosmetic ground-ring decal that tracks `CurrentTarget`; does NOT go through the action pipeline.

**Per-frame hot path is cheap and WASM-safe.** Query `existing: Query<(Entity, &TrackingTarget)>` holds 0 or 1 entity in practice. Per frame it reads one `GlobalTransform`, and only writes `Transform.translation.{x,z}` when XZ moved > 0.001 (epsilon-guarded — respects the change-detection discipline rule). No per-frame heap allocations. The only per-frame `format!` is the `Name::new(format!(...))` which is gated behind `current_target.is_changed()` (rare), not per-frame.

**Local cache lifecycle is correct across scene changes.** `cached: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>`.
- On scene load, `scene_loader.rs` (~line 1190) inserts `LoadedTargetIndicator(resolved)` → `is_changed()` fires → rebuild.
- On `Action::LoadScene`, `action_executor.rs` (~line 50) inserts `LoadedTargetIndicator(None)` → `is_changed()` fires → `.map()` over `None` sets `*cached = None`. So stale handles from the previous scene are dropped on transition. No leak, no stale-handle reuse.

**Blend + depth_bias parity confirmed.** Material uses `AlphaMode::Blend`, `unlit: true`, `depth_bias: 64.0`, `double_sided: true`, `cull_mode: None`. This matches the established decal pattern (`decal.rs` uses Blend + `depth_bias: 128.0`) and particle_renderer (Blend/Add). The Blend pipeline is already warmed by the decal/particle capabilities, so no NEW WebGPU pipeline compile is introduced by this material variant on a scene already using decals/blend particles. depth_bias is a documented existing pattern in this engine (decal=128, indicator=64) — treated as relative ordering, not a per-platform unit concern here.

See [[project_wasm_size]] (added zero deps).

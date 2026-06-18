---
name: target-indicator-system
description: target_indicator_system per-frame cost, two-cache (mesh + per-colour mats) Local lifecycle, colour resolution, and Blend/depth_bias parity with decal capability
metadata:
  type: project
---

`target_indicator_system` (capabilities/target_indicator.rs) runs in `Update` (NOT in the chained interpreter set). Cosmetic ground-ring decal that tracks `CurrentTarget`; does NOT go through the action pipeline.

**Per-frame hot path is cheap and WASM-safe.** The XZ tracking loop (lines ~83-100) runs BEFORE the `current_target.is_changed()` early-return (line 102). It touches only `existing` / `global_transforms` / `transforms` / `commands` — it does NOT touch the colour caches or `prefab_catalog` / `prefab_keys`. Query `existing: Query<(Entity, &TrackingTarget)>` holds 0 or 1 entity in practice. Per frame: one `GlobalTransform` read; writes `Transform.translation.{x,z}` only when XZ moved > 0.001 (epsilon-guarded — respects change-detection discipline). No per-frame heap allocations. The `Name::new(format!(...))` is gated behind `current_target.is_changed()` (rare).

**Two-cache design (rewritten 2026-06, was single (mesh,mat) pair):**
- `cached_mesh: Local<Option<Handle<Mesh>>>` — single Plane3d, radius-driven, colour-independent. Rebuilt only on `indicator_cfg.is_changed()`.
- `cached_mats: Local<HashMap<[u32;4], Handle<StandardMaterial>>>` — keyed by `f32::to_bits()` of each RGBA channel. One entry per distinct resolved colour seen this scene. `or_insert_with` creates a StandardMaterial on miss only.
- Both cleared on scene change (inside `indicator_cfg.is_changed()` branch). `HashMap::clear()` drops the strong `Handle<StandardMaterial>` values correctly → no asset leak. Local retains HashMap capacity across frames (good — no realloc on repeat switches). Same scene-teardown lifecycle as before: executor inserts `LoadedTargetIndicator(None)` on `Action::LoadScene` → is_changed fires → caches cleared.

**Colour resolution** `resolve_indicator_color` (pure fn): three-tier precedence — prefab `indicator_color` > prefab `indicator_category` looked up in `cfg.named_colors` > scene `cfg.color`. Does one `Query<&PrefabKey>::get` + two `HashMap::get`. NO catalog clone. Runs only on target switch.

**`Res<LoadedPrefabCatalog>` added as 2nd reader** — shared `Res` borrow; action_executor also reads it as `Res`. Unlimited concurrent readers, no double-borrow/ResMut conflict. Clone-able but not cloned here.

**Pipeline:** multiple StandardMaterial instances differing only in `base_color` do NOT cause extra WebGPU pipeline compiles — specialization keys on layout/shader permutation (vertex layout + AlphaMode::Blend + unlit + cull_mode), not uniform values. All indicators share one Blend/unlit/cull_mode:None variant, already warmed by decal/particle capabilities (decal=depth_bias 128, indicator=64). Zero new pipeline compile.

**Binary size:** the new `HashMap<[u32;4],Handle<StandardMaterial>>` monomorphization adds a few KB at most; zero new deps. Noise vs ~90.7 MB. See [[project_wasm_size]].

Nit: `color_key` uses raw `to_bits()` so +0.0/-0.0/NaN would key inconsistently — fine because colours are well-behaved RON config values, never computed.

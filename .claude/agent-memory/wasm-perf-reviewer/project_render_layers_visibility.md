---
name: render-layers-visibility
description: Bevy RenderLayers perf profile for per-viewport ring visibility — spawn-time-only, folds into existing check_visibility pass, warmup/default-layer gotchas
metadata:
  type: project
---

Assessed 2026-07-19 for `planning/features/per_viewport_target_ring_visibility.md` (plan-review, pre-code). Feature: opt-in `SplitScreenDef.ring_visibility: OwnViewportOnly` uses Bevy `RenderLayers` so a split player's target ring renders only in their own viewport. Spawn-time-only component inserts on split `OrbitCamera`s (`entity_spawner.rs:706-721`) and ring entities (`target_indicator.rs:~170`). No new per-frame system proposed. Reserved layers: inspector uses 31 (isolation); this feature reserves 1..=MAX_SPLIT_PLAYERS(4).

**Perf: near-zero. Do not flag.**
- `RenderLayers` visibility resolves inside the EXISTING `check_visibility` pass (VisibilitySystems::CheckVisibility, PostUpdate). No new pass. It's a bitmask AND (`RenderLayers::intersects`) added to the per-view loop that already does frustum culling — cheaper than the cull math already there.
- Spawn-time insert = one-time archetype move. Zero per-frame writes, zero change-detection churn.
- 4-way split already runs 4 view passes (inherent, see [[split-screen-viewport]]); the layer test per entity per view is trivial on top. ≤4 ring entities tagged. No scaling concern.
- Zero binary-size impact: `RenderLayers` already compiled (inspector.rs uses it); no new dep, no new monomorphization. ~58 MB unchanged.
- CPU-side gate — identical on WebGL2 and WebGPU. No backend feature dependency. test_web.py (WebGL2 build) validates it fine.

**Warmup interaction (WASM gotcha, currently benign):** `pipeline_warmup_system` force-compiles pipelines via `NoFrustumCulling` for 4 frames. `NoFrustumCulling` overrides FRUSTUM culling but NOT `RenderLayers` — a layer-1-only entity stays invisible to a layer-{0,2} camera even during warmup. So [[split-screen-viewport]]'s "every mesh force-visible to every camera during warmup" claim breaks for layer-restricted entities. Harmless HERE because rings reuse the already-warm Blend/unlit/cull_mode:None StandardMaterial variant (see [[target-indicator-system]]) = zero new pipeline variants. RULE: never put a mesh with a NEW/unique pipeline variant on a restricted-only layer, or warmup misses it → 300-2000ms first-reveal stall.

**PLAN DEFECT found (correctness, in the RenderLayers scheme):** plan put rings on `layer(N)` ONLY (excluding 0) and left the merged/party camera componentless (= default layer 0). `RenderLayers::default()` is layer 0; {0} AND {1} = empty → merged/party camera CANNOT see any ring in OwnViewportOnly mode, contradicting the plan's own acceptance criteria (merged state = all rings visible). Fix (still spawn-time-only, still cheap): give the party/merged camera explicit `RenderLayers::from_layers(&[0,1,2,3,4])` in OwnViewportOnly mode; stays componentless in AllViewports. General lesson: a camera with no RenderLayers sees layer 0 ONLY, not "everything."

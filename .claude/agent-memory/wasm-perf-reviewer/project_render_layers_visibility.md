---
name: render-layers-visibility
description: Bevy 0.18 RenderLayers perf profile (verified post-implementation) — spawn-time-only, net win in check_visibility, no heap alloc, warmup/MAX_SPLIT_PLAYERS gotchas
metadata:
  type: project
---

Feature `SplitScreenDef.own_viewport_only: bool` — opt-in so a split player's target ring renders only in their own viewport. Plan-reviewed 2026-07-19, **implementation verified against real code 2026-07-31** (`feature/target-ring-visibility`). Reserved layers: 1..=`MAX_SPLIT_PLAYERS`(4) for rings; inspector uses 31.

**Perf: verified near-zero, marginal net WIN. Do not flag.**
- `RenderLayers` insert sites are exactly 4, ALL spawn-time: `target_indicator.rs` (inside the `Changed<PlayerTarget>` loop, after the per-frame XZ-follow loop — never touched by the follow/despawn path), `entity_spawner.rs` x2 (`spawn_players_and_camera` dynamic+static loops, `spawn_split_camera_for_player`), `camera.rs` (`spawn_party_orbit_camera`). Grep `RenderLayers` in `crates/` to re-confirm — no per-frame insert/remove anywhere.
- **The layer test is ALREADY on the hot path unconditionally.** `bevy_camera-0.18.0/src/visibility/mod.rs:643-646` does `maybe_entity_mask.unwrap_or_default()` then `view_mask.intersects(entity_mask)` for every entity in every view. Adding the component changes only `None`→`Some` (one sparse-set fetch for ≤4 rings, ≤3 cameras). And `intersects` runs **before** frustum culling, so a layer-mismatched ring short-circuits *earlier* than it used to → strictly less work, plus up to 3 fewer ring draw calls/frame in 4-way split.
- `RenderLayers` is `SmallVec<[u64; 1]>` (`INLINE_BLOCKS = 1`). Layers 0..=4 all fit the single inline u64 → **zero heap allocation**, even for `from_layers(&[0,1,2,3,4])`.
- `Res<TargetRingVisibilityMode>` added to `target_indicator_system` and `drain_spawn_queue_system`: fieldless `Copy` enum, compared by discriminant (`*res == Variant`), no clone/alloc. In `drain_spawn_queue_system` the compare sits inside the hot-join branch, unreachable when the queue is empty (`pop_front()` → `break`). Nothing gates on its `is_changed()`, so the `insert_resource` churn at scene load has no ripple.
- `dynamic_split_screen_system` only toggles `Camera.is_active` — it never respawns cameras, so it can't drop or re-churn `RenderLayers`.
- CPU-side gate only — identical on WebGL2 and WebGPU, no backend feature dependency, no wgpu limit involved. Rings are `unlit: true` so being off the light's layer 0 costs nothing (and correctly excludes them from the shadow pass).
- Zero binary-size impact: no dep change (`Cargo.toml`/`Cargo.lock` untouched); `RenderLayers` + `intersects` + `Default` are already linked via `check_visibility`. New instantiations (`layer`/`with`/`from_layers`/`insert::<RenderLayers>`) are KB-scale. See [[project-wasm-size]].

**Warmup interaction (WASM gotcha, currently benign):** `pipeline_warmup_system` (`lib.rs`) force-compiles pipelines by inserting `NoFrustumCulling` on `Mesh3d` entities for 4 frames. `NoFrustumCulling` overrides FRUSTUM culling but NOT `RenderLayers` (see the ordering at mod.rs:643 vs :658) — a layer-1-only entity stays invisible to a layer-{0,2} camera even during warmup. Harmless HERE because rings reuse the already-warm Blend/unlit/cull_mode:None `StandardMaterial` variant (see [[target-indicator-system]]) = zero new pipeline variants. RULE: never put a mesh with a NEW/unique pipeline variant on a restricted-only layer, or warmup misses it → 300-2000ms first-reveal stall.

**Invariant (was a plan defect, now FIXED in code):** a camera with no `RenderLayers` sees layer 0 ONLY, not "everything." The party/merged camera therefore needs explicit `RenderLayers::from_layers(&[0,1,2,3,4])` in `OwnViewportOnly` mode, else it renders **zero** rings. Latent coupling: that literal is hand-derived from `MAX_SPLIT_PLAYERS = 4` (`capabilities/camera.rs:324`) — raising `MAX_SPLIT_PLAYERS` silently makes the merged camera blind to rings 5+. Flag this if anyone touches that constant.

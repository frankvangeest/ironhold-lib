---
name: render-layers-reserved-scheme
description: RenderLayers facts not captured in CLAUDE.md — layer-restricted meshes are dropped from layer-0 lights' shadow passes, RenderLayers is heap-free below layer 64, and the layer scheme has no single owning module
metadata:
  type: project
---

The reserved-layer scheme itself (1-4 per split player, 31 inspector, party-camera union) IS
documented in `crates/ironhold_core/src/CLAUDE.md`. These are the parts that are not, learned
during the `per_viewport_target_ring_visibility` review (2026-07-31):

**Lights are layer-filtered against the MESH, not the camera.**
`check_point_light_mesh_visibility` (and the directional equivalent) in `bevy_light-0.18.0/src/lib.rs`
intersects the *light's* `RenderLayers` (default = layer 0) with each candidate mesh's layers. So a
mesh restricted to layer N **only** is excluded from every layer-0 light's shadow pass. Harmless for
the target ring (`unlit: true`, so it needs no lighting; and it stops contributing a flat-plane
shadow, arguably an improvement). But it means **the reserved-layer scheme only works for unlit
cosmetic overlays** — restricting a *lit* prefab to a player layer would render it unlit and
shadowless. Any future `RenderLayers` consumer needs matching layer unions on the lights.

**Cost is negligible and WASM-neutral.** `RenderLayers(SmallVec<[u64; INLINE_BLOCKS]>)` with
`INLINE_BLOCKS = 1` (`bevy_camera-0.18.0/src/visibility/render_layers.rs:20-23`) — layers 0-63 are
one inline `u64`, zero heap allocation, ~32 bytes. Filtering happens in `check_visibility`, which
already iterates entity × view, so there is no new pass, no GPU state, no pipeline variance:
identical behavior on WebGPU and WebGL2, no binary-size impact. Don't treat a `RenderLayers`
addition as a perf risk; the risk is always *visibility semantics*, never cost.

**The scheme has no single owning module.** `MAX_SPLIT_PLAYERS`/`PLAYER_LABEL_COLORS` live in
`capabilities/camera.rs`, `TargetRingVisibilityMode` lives in `runtime/scene_manager/mod.rs`, and
the mapping `1 + player_index % MAX_SPLIT_PLAYERS` is written out longhand at three sites
(entity_spawner's dynamic loop, `spawn_split_camera_for_player`, `target_indicator_system`) with the
party union as a hardcoded `&[0,1,2,3,4]` literal in `camera.rs`. Raising `MAX_SPLIT_PLAYERS` past 4
silently desyncs the party union — the merged view loses the extra players' rings, the exact defect
plan review caught for the componentless-party-camera draft. Recommended fix (raised, not yet
applied): `ring_layer_for_player()` + `all_ring_layers()` helpers next to `MAX_SPLIT_PLAYERS` in
camera.rs.

Related: [[split-screen-and-shared-mouse]], [[camera-architecture]],
[[render-only-reactive-capabilities]], [[scene-load-resource-threading]].

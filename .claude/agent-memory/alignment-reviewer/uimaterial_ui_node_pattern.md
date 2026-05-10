---
name: UiMaterial-driven UI node pattern (StatRadar precedent)
description: How a shader-backed UI node like StatRadar is wired through schema, scene_loader, capability, and lib.rs registration without breaking the data-driven contract
type: project
---

When a UI node renders via a custom WGSL shader rather than vanilla `BackgroundColor`, the engine has settled on this five-touchpoint pattern (StatRadar is the precedent at `2026-05-10`):

1. **Schema variant** in `schema/scene_v2.rs::UiNodeDef` — new struct must implement all five `UiNodeDef` helper methods (`id`, `size`, `position`, `absolute`, `align`) and use `#[serde(deny_unknown_fields)]` + named `default_*` fns for every visual knob (colors, widths, sizes).

2. **Capability module** at `capabilities/{name}.rs` containing:
   - `Asset + AsBindGroup` material struct with a single `RadarUniforms`-style 16-byte-aligned `ShaderType` uniform (vec4 fields only — bare `f32`/`Vec3` triggers `BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED` panics on WebGPU).
   - `impl UiMaterial` returning the engine-side shader `ShaderRef`. Hardcoding the engine shader path here is **acceptable** — same pattern as `terrain_material.rs` — because the shader is a fixed engine implementation, not designer content. Only "user content" shaders (like `CustomMaterial`'s shaders, listed in `assets.ron`) must come from RON.
   - Component (e.g. `StatRadarNode`) carrying the per-instance binding (stat keys list).
   - Update system reading `LoadedStats` (or whatever data source) and writing the material uniform with **change-detection guard** (compare-then-write — unconditional writes mark the material changed every frame and re-trigger render work).
   - `Plugin` adding `UiMaterialPlugin::<XxxMaterial>::default()`.

3. **`SceneMaterialParams`** in `runtime/scene_manager/mod.rs` — add `pub xxx: ResMut<'w, Assets<XxxMaterial>>` so `scene_loader` can mint handles. This is a `SystemParam` struct so it does not count against the 16-param limit.

4. **`scene_loader.rs`** — pre-create the material handles **before** the `with_children` UI closure (handles are owned and can be moved into the FnMut closure; `mats.xxx` cannot be borrowed inside it). Then the `match` arm in `spawn_ui_element_node` looks up the pre-created handle by id and spawns `(node, MaterialNode(handle), MarkerComponent)`.

5. **`lib.rs`** — register the `Plugin` and add the update system to `Update` schedule.

Designer-reachability test that confirms this pattern is aligned: a designer adds `XxxNodeDef(...)` to any `*.scene.ron` and the node renders with the configured colors/sizes/data binding — zero Rust changes, zero `assets.ron` edits.

Things to flag if a future UI-material PR deviates:
- New configurable knob (color, threshold, layout) hardcoded in the capability instead of in the schema struct.
- Shader path read from a hardcoded prefix that varies per project (would need an `assets.ron` entry instead).
- Update system writing the uniform every frame without a change-guard.
- Capability omitted from `lib.rs` registration (UI parses but the node is invisible — fail-open to a `warn!` is acceptable, but the registration omission must be caught).
- Material handle leaked into the `with_children` closure by reference, causing borrow-checker fights — the pre-creation pattern is the canonical fix.

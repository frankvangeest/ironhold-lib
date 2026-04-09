# ironhold_core — Rust Source Rules

## WebGPU 16-byte alignment
Custom GPU-bound structs (e.g., `TerrainMaterial`) **must** use 16-byte aligned uniform buffer layouts. Violating this causes `BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED` panics in web builds. Verify `AsBindGroup` mappings distinguish Uniform vs. Storage buffers per Bevy 0.18 expectations.
- Use `Vec4` (16 bytes) for all uniform fields; never bind a bare `f32`, `Vec2`, or `Vec3`.
- `CustomMaterialUniforms` (4 × Vec4 = 64 bytes) and `TerrainMaterial.uv_scale` (Vec4 padded) already comply — keep them that way.

## WGSL is the first-class shader language
All shaders in this project are authored in WGSL. WGSL is the native language of WebGPU and runs identically on desktop (wgpu) and browser (WebGPU) — zero transpilation cost and consistent output on all platforms.

**When writing or reviewing shader code:**
- Shared (reusable) shaders → `assets/shared/shaders/`, named `custom_*.wgsl`.
- Project-specific shaders → `assets/projects/{name}/shaders/`.
- All custom fragment shaders must declare the full `CustomMaterial` binding contract (see `docs/25_custom_shaders.md`). Missing bindings cause WebGPU validation errors, not panics.
- `TonyMcMapface` and `BlenderFilmic` tonemapping are excluded because they require a LUT texture. Do not add LUT-dependent shaders.
- `CustomMaterial` currently overrides the **fragment shader only**. Vertex shader override is planned but not yet implemented — do not attempt to swap the vertex shader via `specialize()`.
- Always test WGSL changes in a web build (`python test_web.py`). WebGPU validates binding interfaces strictly; native wgpu is more permissive and will not catch all errors.

See `docs/25_custom_shaders.md` for the full shader authoring guide.

## Physics & movement must use `FixedUpdate`
All player movement, physics processing, and camera-follow logic must run in `FixedUpdate`. Using `Update` for physics-driven movement causes stuttering.

## Terrain generation is async
Terrain mesh generation is compute-heavy. Always use Bevy's `AsyncComputeTaskPool` and poll `Task` components — never block the main thread.

## Inspector isolation
`bevy_egui` inspector and game UI are strictly separated. The inspector renders on its own camera/layer; never mix it with the main game UI camera. Data structs that should be visible in the inspector must be conditionally exposed using `#[cfg_attr(feature = "inspector", ...)]` attributes — see existing components for the pattern.

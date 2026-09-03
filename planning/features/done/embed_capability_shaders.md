# Feature: Embed capability shaders & fix hardcoded shared asset paths

_Status: Ready_
_Planned at: `d33e410` (2026-06-17)_

## What

Four capabilities (`stat_radar`, `foliage`, `flame_material`, `particle_renderer`) reference their GPU shaders via hardcoded `"shared/shaders/..."` `ShaderRef` strings, which are loaded from disk at runtime. A project without `assets/shared/` will get WebGPU pipeline errors the moment any of these capabilities renders. Additionally, `foliage.rs` falls back to constructing `"shared/textures/{key}.png"` paths in code when a catalog lookup misses — bypassing the `AssetCatalog` entirely.

This feature replaces runtime file references with compile-time `include_str!()` embeds (matching the existing `terrain.wgsl` and `custom_material_default.wgsl` pattern) and removes the fabricated texture fallback path.

## Why

- The WASM library must be usable without `assets/shared/` present. A project that doesn't use foliage or particles should not have latent GPU errors waiting to fire.
- Users should be able to copy a shader into their own project and use it without forking the engine — but the current `ShaderRef` string is hardcoded in Rust, making per-project overrides impossible.
- The `foliage.rs` texture fallback silently fabricates a path outside the catalog, which violates the core invariant that all asset paths flow through `assets.ron`.

## Approach

### Shaders to embed (`include_str!`)

Each of the four sites follows the same pattern as `terrain.rs`:

1. Declare a `static HANDLE: Handle<Shader> = Handle::weak_from_u128(<uuid>)`.
2. Add a startup system `fn setup_shader(mut shaders: ResMut<Assets<Shader>>)` that calls `Shader::from_wgsl(include_str!("../../../../assets/shared/shaders/foo.wgsl"), "shared/shaders/foo.wgsl")` and inserts it at the stable handle.
3. Change the `Material`/`UiMaterial` impl's `fragment_shader()` / `vertex_shader()` to return `ShaderRef::Handle(HANDLE.clone())` instead of a path string.

Files and shaders:

| Capability file | Shader(s) to embed |
|---|---|
| `capabilities/stat_radar.rs` | `custom_stat_radar.wgsl` |
| `capabilities/foliage.rs` | `foliage.wgsl`, `foliage_prepass.wgsl` |
| `capabilities/flame_material.rs` | `custom_flame_particle.wgsl` |
| `capabilities/particle_renderer.rs` | `pool_flame_particle.wgsl` |

### Foliage texture fallback fix (`foliage.rs:122`)

Remove the `unwrap_or_else(|| format!("shared/textures/{}.png", ...))` line. Replace with:
- If the catalog key is missing: `warn!` once and use a 1×1 white `Image` handle (can reuse Bevy's built-in `Handle::<Image>::default()` or `Color::WHITE` default material texture).
- Add the missing key to the CLI `validate` cross-reference check so it's caught at authoring time, not at runtime.

### No schema changes

These are internal runtime changes. No new RON fields, no changes to `schema/`. The `alignment-reviewer` prompt has already been updated to flag this class of issue in future reviews.

## Tasks

- [ ] Embed `custom_stat_radar.wgsl` — stable handle, startup system, update `ShaderRef`
- [ ] Embed `foliage.wgsl` + `foliage_prepass.wgsl` — same pattern
- [ ] Embed `custom_flame_particle.wgsl` — same pattern
- [ ] Embed `pool_flame_particle.wgsl` — same pattern
- [ ] Fix `foliage.rs:122` — remove fabricated `shared/textures/` fallback; warn + white fallback
- [ ] CLI validate: add foliage `leaf_texture` key cross-reference check
- [ ] Tests — verify existing integration tests still pass; add a test that a scene with foliage but a missing leaf_texture key produces a warning (not a crash)
- [ ] Docs — update `docs/25_custom_shaders.md` to note that engine-internal shaders are binary-embedded; update `crates/ironhold_core/src/CLAUDE.md` shader section

## Open questions

- None — approach is settled by architect review.

## Acceptance criteria

- A project with no `assets/shared/` directory launches without any GPU errors or panics, even with `StatRadarPlugin`, `FoliagePlugin`, `FlameParticleMaterialPlugin`, and `ParticleRendererPlugin` registered.
- A scene that uses foliage with a catalog-resolved texture renders correctly.
- A scene that uses foliage with a missing leaf_texture catalog key emits a `warn!` and renders with a white fallback, instead of silently constructing a `shared/textures/` path.
- `cargo run -p ironhold_cli -- validate` reports a missing foliage texture key as a cross-reference error.
- All existing tests pass.
- WASM release binary size does not grow by more than 500 KB (shaders are small text files).

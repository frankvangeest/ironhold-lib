---
name: shader-resolution-pattern
description: How shaders should be resolved in ironhold_core — engine-owned (embed) vs designer-authored (catalog); the CUSTOM_MATERIAL_FALLBACK_HANDLE gold standard
metadata:
  type: project
---

There are two distinct shader categories in ironhold_core, and they have different correct resolution mechanisms. Do not conflate them.

**Engine-owned shaders** (radar widget, foliage renderer, flame/particle pool materials): the designer authors *parameters* (e.g. `FoliageMaterialDef.toon_bands`, colors), not the GPU program. The engine owns the WGSL. These must be embedded via `include_str!` + a fixed UUID `Handle<Shader>` registered in a `Startup` system — the `CUSTOM_MATERIAL_FALLBACK_HANDLE` pattern in `capabilities/custom_material.rs` (const at line ~11, `setup_custom_material_fallback_shader` at ~205). The `Material::fragment_shader()` impl returns `HANDLE.into()`, NOT a `"shared/shaders/*.wgsl"` string literal.

**Designer-authored shaders** (`CustomMaterial`): resolved through a per-material `shader: Option<String>` field at material-build time in `material_factory.rs` (~line 96), `asset_server.load::<Shader>(path)`, with the embedded magenta handle as fallback. Per-instance shader swap happens in `specialize()` via a bind-group key (`CustomMaterialKey`) because `fragment_shader()` is a static fn with no catalog access.

**Why embed engine shaders:** `Material::fragment_shader()` returning a `"shared/..."` ShaderRef string only works because Bevy compiles the pipeline lazily (first render of that material). In a no-`assets/shared/` project, the moment that capability is used you get a runtime asset-not-found. Embedding removes the runtime file dependency entirely (compile-time data) and also sidesteps the WASM HTTP-fetch-on-first-use path. The custom_material.rs doc comment explicitly notes embedding is "important for WASM builds."

**Anti-pattern to flag (FIXED as of this update):** hardcoded shader path string literals inside `impl Material`/`UiMaterial` (`fragment_shader`, `vertex_shader`, `prepass_*`, `ShaderRef::from("...")`). As of 2026-06 these existed at stat_radar.rs:58, foliage.rs:51-61, flame_material.rs:47, particle_renderer.rs:168 (alignment-reviewer did not catch these at the time — its anti-pattern list was RON/catalog-path focused). All four now follow the recommended embed pattern: `fragment_shader()` returns a fixed UUID `Handle<Shader>` (`STAT_RADAR_SHADER_HANDLE`, etc.), registered from `include_str!("../../../../assets/shared/shaders/*.wgsl")` in a `Startup` system — verified in `stat_radar.rs`, `foliage.rs`, `flame_material.rs`, and `particle_renderer.rs`. This is now also documented as the required pattern in `crates/ironhold_core/src/CLAUDE.md`'s "Engine-internal shaders" section. Still worth re-grepping `ShaderRef::from(` across `capabilities/` on any new engine-owned material to confirm a future addition doesn't reintroduce this.

**Foliage texture fallback footgun — FIXED.** foliage.rs's `foliage_setup_system` now does a proper catalog lookup (`asset_catalog.0.textures.get(&def.material.leaf_texture)`), and `warn!`s + skips spawning the foliage entity entirely when the key isn't found, instead of fabricating a `format!("shared/textures/{}.png", key)` path. This matches the "never fabricate asset paths, warn + skip" rule now stated in `crates/ironhold_core/src/CLAUDE.md`.

**How to apply:** When advising on any new GPU material capability, default to the embed pattern for engine-owned shaders. Only add a catalog/override path when a designer genuinely authors the shader. Related: [[core-architectural-decisions]] (asset-paths-via-catalog), [[fragile_modules]] (WebGPU alignment).

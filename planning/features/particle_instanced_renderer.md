# Feature: Particle System v2 — 1. Instanced Renderer

_Status: Done_
_Planned at: `2cc61ca` (2026-05-19)_
_Part of: see `planning/features/particle_system_v2.md` for the full v2 overview_

## What

Replace per-entity particle rendering with a GPU-instanced approach. All live particles of
the same material variant render in a single draw call instead of one draw call per particle.

## Why

The current model — one `Mesh3d` + unique `Handle<Material>` per particle — breaks Bevy's
automatic batching. 40 players × 10 particles each = 400 draw calls per frame. WebGPU
is particularly sensitive to draw call count; this is a hard performance wall that must
be resolved before any other v2 features are worth building.

## Approach

**Per-frame instance buffer** — the CPU simulation tick writes a flat
`Vec<ParticleInstance>` each frame. One buffer per material variant. A single
instanced draw call renders the whole buffer.

```rust
// 16-byte aligned, passed as per-instance vertex attributes
#[repr(C)]
struct ParticleInstance {
    translation: Vec3,
    rotation_rad: f32,   // Z-axis billboard rotation
    color: [f32; 4],     // RGBA linear
    size_xy: [f32; 2],   // width × height (non-uniform scale)
    uv_offset: [f32; 2], // for flipbook / flame scroll
}
```

**Material variants** — one `SpecializedMeshPipeline` specialization per blend mode +
shader combination:

| Variant | Alpha | Shader | Use |
|---|---|---|---|
| `Additive` | `AlphaMode::Add` | `instanced_particle.wgsl` | Fire, magic, electricity |
| `Blend` | `AlphaMode::Blend` | `instanced_particle.wgsl` | Smoke, cloud, soft auras |
| `FlameDistort` | `AlphaMode::Add` | `instanced_flame.wgsl` | UV distort + scroll flame |
| `Glow` | `AlphaMode::Add` | `instanced_particle.wgsl` + `glow` flag | Shader radial gradient |

**Resource structure:**
- `ParticlePool` resource — owns `Vec<ParticleState>` (CPU state per alive particle)
- `ParticleInstanceBuffers` resource — one `Buffer` per variant, uploaded each frame
- Simulation runs in `Update`; render extraction runs in Bevy's render schedule

**Bevy 0.18 integration:**
- Custom `RenderPlugin` registers the pipeline and `RenderCommand`
- `ExtractResourcePlugin::<ParticleInstanceBuffers>` copies to render world each frame
- The shared `Handle<Mesh>` (unit quad) is retained; sizing via `size_xy` instance data

**Files:**
- `capabilities/particle_renderer.rs` (new) — CPU pool, simulation, buffer upload
- `capabilities/particle.rs` — stripped to public API types + `PendingParticleEffects`
- `assets/shared/shaders/instanced_particle.wgsl` (new)
- `assets/shared/shaders/instanced_flame.wgsl` (new, extends flame distort/scroll)

## Tasks

- [ ] Design `ParticleInstance` struct (verify 16-byte alignment for WebGPU)
- [ ] Implement `InstancedParticleMaterial` with `AsBindGroup` (texture + uniforms)
- [ ] Write `instanced_particle.wgsl` — billboard with per-instance color/size/rotation/uv
- [ ] Write `instanced_flame.wgsl` — above + UV distort/scroll uniforms
- [ ] Implement `ParticlePool` CPU simulation (velocity, gravity, turbulence, color lerp, size lerp)
- [ ] Port `drain_particle_effects_system` to write into pool (no per-entity spawning)
- [ ] Implement render extraction: pool → instance buffer upload each frame
- [ ] Register all 4 pipeline variants in warmup system
- [ ] Remove `ParticleMeshCache` (mesh now lives in renderer)
- [ ] Update integration tests (particle count assertions, no longer entity-count based)
- [ ] Update CLAUDE.md warmup section — list 4 variants instead of 2
- [ ] Benchmark: confirm draw call count in Chrome DevTools → target ≤ 4 for 500 particles

## Open questions

- **Vertex buffer vs storage buffer** for instance data: `VertexBuffer` (explicit stride, better
  WebGPU baseline compatibility) vs `StorageBuffer` (flexible size, simpler WGSL). Lean
  toward `VertexBuffer` for safety until WebGPU storage buffer support is confirmed across
  target browsers.
- **Max instance count**: pre-allocate a fixed-size buffer (e.g. 4096 entries) or resize
  dynamically? Fixed is simpler and avoids mid-frame reallocations.

## Acceptance criteria

- 500 simultaneous billboard particles render in ≤ 4 draw calls (one per variant used)
- All existing `particles_demo` effects render correctly; `test_web.py` screenshot baselines match
- No per-particle `Handle<StandardMaterial>` or `Handle<FlameParticleMaterial>` allocations
- Frame time in particles_demo WASM build does not increase vs. the current system

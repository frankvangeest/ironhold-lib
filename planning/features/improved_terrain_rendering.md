# Feature: Improved Terrain Rendering

_Status: Draft_
_Planned at: `e2b096b` (2026-06-15)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| 1+2 | UV elimination + U16 indices; mesh chunking — unblocks terrain snap + streaming | Queued | — |
| 3+4 | GPU-derived XZ positions; compressed normals _(gated on Phase 0 WebGPU PoC)_ | Queued | — |

## What

Reduce terrain GPU memory footprint by ~25–50 % and eliminate the WASM first-frame stall by restructuring the terrain mesh pipeline in four discrete phases. Designers gain a per-scene `chunk_size` knob; everything else is transparent to RON authors.

Research basis: `planning/investigations/Terrain-rendering-optimisations-investigation.md`
Physics feasibility research: system-architect analysis of `wgrapier3d` (v0.2.0, Nov 2024 — deferred; see Icebox).

---

## Why

- The current generator stores redundant UV coordinates that can be derived from world XZ at zero quality cost.
- `chunk_size` exists in `TerrainConfigV2` but is unused — the whole mesh is one draw call with no culling, no async incremental generation, and a single monolithic Rapier collider rebuilt on every scene load.
- `AsyncComputeTaskPool` degrades to `block_on` on the main WASM thread, causing 100–500 ms jank on first frame for large heightmaps.
- The chunking work directly unblocks two queued backlog items: terrain chunked streaming and terrain snap.

---

## Approach

### Phase 0 — Custom vertex shader feasibility spike (gate for Phases 3–4)

Before committing to GPU-derived XZ positions, prove that a custom `MeshPipeline`-override vertex shader round-trips correctly through Bevy's WebGPU backend. Write the minimal WGSL + Rust wiring, render one terrain quad, confirm no WebGPU validation errors in Chrome and Firefox. If it fails, Phases 3–4 are cut; Phases 1–2 ship on their own.

### Phase 1 — UV elimination + U16 index buffer

**Vertex layout change:**

| Before | After |
|--------|-------|
| Position `vec3<f32>` | Position `vec3<f32>` (unchanged) |
| Normal `vec3<f32>` | Normal `vec3<f32>` (unchanged) |
| UV `vec2<f32>` | _(dropped)_ |
| U32 index buffer | U16 index buffer (chunks ≤ 65 535 verts) |

UV coordinates are derived in the terrain fragment shader from world XZ divided by tile size:

```wgsl
// terrain_material.wgsl — replace uv input with derived coords
let uv = vec2<f32>(in.world_position.x, in.world_position.z) / terrain.tile_size;
```

`terrain_material.rs` passes `tile_size` via the existing uniform block. No schema change needed — `tile_size` is already a field on `TerrainConfigV2`.

**Expected gain:** ~25 % reduction in per-mesh vertex memory (removes 8 bytes per vertex).

### Phase 2 — Mesh chunking (highest-value structural change)

Wire up the currently ignored `chunk_size: u32` on `TerrainConfigV2`. The terrain capability splits the heightmap into N×N chunks at load time, each a separate `Mesh` + `Handle<Mesh>` + `Handle<TerrainMaterial>` entity.

Key design decisions:

- **Frustum culling** — each chunk entity gets an `Aabb` computed from its min/max height values; Bevy culls it for free.
- **Incremental async generation** — spawn one `AsyncComputeTaskPool` task per chunk; `poll_terrain_tasks_system` drains one result per frame (or a configurable `CHUNKS_PER_FRAME` budget). Eliminates the first-frame stall entirely on WASM.
- **Per-chunk Rapier collider** — replace the single `Collider::heightfield` with one `Collider::trimesh` per chunk, built from the same chunk vertex data. Colliders spawn as tasks complete; no full rebuild on scene reload unless the heightmap path changes.
- **CPU height-array** — a single `Arc<Vec<f32>>` stored in a `TerrainHeightmap` resource holds the raw heights for the entire scene. All CPU callers (terrain snap, Rapier trimesh builder) read from this shared array. The GPU and CPU are driven by identical data — they cannot desync.

New `TerrainConfigV2` fields (all `#[serde(default)]`):

```ron
// existing field, now honoured:
chunk_size: 32,          // vertices per chunk edge; 0 = single mesh (backwards-compatible)

// new optional field:
chunks_per_frame: 2,     // how many chunk tasks to drain per frame (default 2)
```

**Unblocks:** terrain snap (`snap_to_terrain: true`), terrain chunked streaming (future per-chunk load/unload builds on this foundation).

### Phase 3 — GPU-derived XZ positions (gated on Phase 0 PoC)

Store only Y (height) per vertex. The vertex shader reconstructs XZ from `vertex_index`, chunk origin, and step size passed via a push-constant or uniform:

```wgsl
@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VertexOutput {
    let col = vid % terrain.chunk_verts;
    let row = vid / terrain.chunk_verts;
    let x = terrain.chunk_origin.x + f32(col) * terrain.step;
    let z = terrain.chunk_origin.z + f32(row) * terrain.step;
    let y = heights[vid];           // storage buffer or vertex attribute
    ...
}
```

CPU retains the `Arc<Vec<f32>>` height array — Rapier and snap queries are unaffected.

**Expected additional gain:** drops X and Z `f32` per vertex (~8 bytes) on top of Phase 1.

### Phase 4 — Compressed normals (gated on Phase 0 PoC)

Encode normals as two `i16` values (octahedral or spheremap encoding) instead of three `f32` values, decoded in the vertex shader:

```wgsl
let n = decode_oct(vec2<f32>(in.normal_enc) / 32767.0);
```

**Expected additional gain:** 4 bytes per vertex (from 12 → 4).

### Combined savings estimate

| Phase | Bytes saved / vertex | Cumulative |
|-------|---------------------|------------|
| 1 — UV drop + U16 idx | 8 | ~25 % |
| 2 — Chunking (no vertex change) | 0 | culling + async win |
| 3 — XZ drop | 8 | ~45 % |
| 4 — Normal compress | 8 | ~60 % |

---

## Physics — wgrapier deferred

`wgrapier3d` v0.2.0 (Dimforge, Nov 2024) is a dynamic-body simulator (broad-phase + Soft-TGS constraint solver) aimed at dense particle/debris workloads. It cannot answer height queries without an async GPU→CPU readback, has no docs, no Bevy integration, and Dimforge has already announced a full rewrite on `rust-gpu`. The CPU height-array is the correct design — not a fallback. Revisit wgrapier when the rust-gpu rewrite ships with real releases and an official Bevy bridge exists. See Icebox entry in `planning/backlog.md`.

---

## Tasks

### Phase 0 — Vertex shader PoC
- [ ] Write minimal `MeshPipeline`-override vertex shader that derives XZ from `vertex_index`
- [ ] Confirm no WebGPU validation errors in Chrome and Firefox WASM builds
- [ ] Document go/no-go verdict in this file before starting Phase 3

### Phase 1 — UV elimination + U16 indices
- [ ] Remove UV attribute from `generate_terrain_mesh_raw` in `terrain.rs`
- [ ] Switch index buffer from `u32` to `u16` (assert chunk vertex count ≤ 65 535)
- [ ] Update `terrain_material.wgsl` to derive UV from `world_position.xz / tile_size`
- [ ] Pass `tile_size` uniform in `terrain_material.rs` (already a `TerrainConfigV2` field)
- [ ] Verify `terrain_demo` renders identically
- [ ] Tests + docs

### Phase 2 — Mesh chunking
- [ ] Honour `chunk_size` in `terrain.rs`; split heightmap into N×N chunks
- [ ] Spawn one async task per chunk; drain via `poll_terrain_tasks_system` at `chunks_per_frame` budget
- [ ] Compute per-chunk `Aabb` for frustum culling
- [ ] Build per-chunk `Collider::trimesh` from chunk vertex data
- [ ] Store `TerrainHeightmap { heights: Arc<Vec<f32>>, width: u32, depth: u32, scale_x: f32, scale_z: f32 }` resource
- [ ] Add `chunks_per_frame: u32` field to `TerrainConfigV2` (`#[serde(default)]`, default 2)
- [ ] Backwards-compat: `chunk_size: 0` (or absent) falls back to single-mesh behaviour
- [ ] Verify no first-frame stall in WASM build of `terrain_demo`
- [ ] Integration test: chunked terrain spawns correct number of chunk entities
- [ ] Tests + docs

### Phase 3 — GPU-derived XZ (after Phase 0 go verdict)
- [ ] Store only Y per vertex; pass chunk origin + step via uniform
- [ ] Update vertex shader to reconstruct XZ from `vertex_index`
- [ ] CPU height-array unchanged — Rapier and snap queries unaffected
- [ ] Tests + docs

### Phase 4 — Compressed normals (after Phase 0 go verdict)
- [ ] Add octahedral encode in `generate_terrain_mesh_raw`
- [ ] Add decode in vertex shader
- [ ] Tests + docs

### Icebox watch item
- [ ] Add one-line Icebox entry for wgrapier in `planning/backlog.md`

---

## Open questions

- Does Bevy 0.18's `MeshPipeline` override API support injecting a custom vertex shader cleanly, or does it require forking `MaterialPlugin`? (Answered by Phase 0 PoC.)
- What is the maximum practical `chunk_size` before per-chunk entity overhead exceeds the culling benefit? (Needs profiling on `terrain_demo` with large heightmaps.)
- Should chunks share a single `TerrainMaterial` handle or get per-chunk instances? (Uniform chunk-origin data suggests per-chunk instances or a storage buffer — decide in Phase 2 implementation.)

---

## Acceptance criteria

- Given a 512×512 heightmap, when `chunk_size: 32` is set, then 256 chunk entities are spawned incrementally across frames with no single-frame stall exceeding 16 ms on WASM.
- Given any heightmap, when the scene loads, then `--check` on the terrain mesh shows UV attributes absent and index buffer is `u16`.
- Given the Phase 0 PoC, when the custom vertex shader runs in Chrome and Firefox, then no WebGPU validation errors appear in the console.
- Given GPU-derived XZ positions (Phase 3), when the player walks on the terrain, then `snap_to_terrain` positions match the visual surface (CPU height-array and GPU displacement are in sync).
- Given a terrain with existing RON that omits `chunk_size`, when the scene loads, then behaviour is identical to before (single-mesh backwards-compat path).

# Feature: Stylized Foliage (Anime / Ghibli-style Trees)

_Status: Draft_
_Planned at: `9b57e75` (2026-06-02)_
_Investigation: `planning/investigations/ghibli-anime-style-tree-generation.md`_

---

## What

A new `kind: Foliage` prefab type that procedurally builds stylized trees and bushes at scene load time — no pre-modelled foliage meshes required. The designer supplies a leaf texture, a palette of three tone colours, cluster shape parameters, and optionally a trunk GLB. The engine builds the leaf card geometry, bakes sphere-mapped vertex normals, and renders everything through a single custom `FoliageMaterial` WGSL shader that handles:

- **Camera-facing billboards** — leaf quads always face the camera; orientation stripped in the vertex shader so the shapes retain their painted look from any angle.
- **Sphere-normal toon shading** — vertex normals are redirected to point outward from the cluster centre rather than from individual quad geometry; the whole cluster shades as a smooth sphere, producing large unbroken light/shadow volumes.
- **Alpha-clip silhouettes** — a brush-stroke alpha texture is hard-discarded at 0.5; no blending, no ordering cost.
- **GPU wind sway** — micro and macro movement driven by `globals.time` entirely in the vertex shader (v2).

---

## Why

Ironhold's current vegetation story is "drop a GLB model in." That works for realistic assets but gives designers no way to create stylized, painterly foliage without access to Blender. Procedural generation from parameters fits the data-driven philosophy — a designer describes the tree in RON and the engine builds it.

The sphere-normal technique is the key insight: without it, individual billboard quads each shade independently and the result looks flat. With sphere normals, an 8-cluster tree shades as a single organic volume with a coherent shadow zone.

---

## RON schema

```ron
// assets/projects/{name}/prefabs/prefabs.ron
"ghibli_oak": (
    kind: Foliage,
    foliage: (
        trunk: Some("models/plants/trunk_with_branches_01"),   // asset catalog key; None for bushes
        clusters: (
            count: 7,
            emitter_radius: 1.5,               // sphere radius for cluster placement
            leaves_per_cluster: 32,            // leaf cards baked into each cluster mesh
            leaf_scale_min: 0.30,
            leaf_scale_max: 0.60,
        ),
        material: (
            leaf_texture: "textures/foliage/leaf_brush_01",  // asset catalog key; alpha-masked PNG
            color_highlight: (0.45, 0.72, 0.25),
            color_midtone:   (0.28, 0.55, 0.15),
            color_shadow:    (0.12, 0.32, 0.08),
            toon_bands: 3,      // 2, 3, or 4
            ao_intensity: 0.4,  // darkens deep splits; 0.0 = off
        ),
    ),
)
```

v2 adds `wind` and `leaf_drop` blocks:

```ron
foliage: (
    // ... existing fields ...
    wind: Some((
        macro_strength: 0.12,   // large branch sway amplitude
        macro_frequency: 0.8,   // sway cycles per second
        micro_strength: 0.04,   // individual leaf flutter amplitude
        micro_frequency: 3.2,   // flutter cycles per second
    )),
    leaf_drop: Some((
        rate: 0.5,              // leaves released per second
        lifetime: 4.0,          // seconds before a dropped leaf despawns
        drift_strength: 0.3,    // horizontal drift as it falls
    )),
)
```

---

## Schema changes (`schema/catalog.rs`)

### New `PrefabKind::Foliage` variant

```rust
pub enum PrefabKind {
    Actor,
    Prop,
    Primitive,
    Foliage,   // ← new
}
```

> **Dependency:** `PrefabKind` is introduced by the [Consistent RON enum casing](consistent_ron_enum_casing.md) migration. Foliage ships after or alongside that migration.

### New structs

```rust
pub struct FoliageDef {
    pub trunk: Option<String>,
    pub clusters: FoliageClustersDef,
    pub material: FoliageMaterialDef,
    #[serde(default)]
    pub wind: Option<FoliageWindDef>,       // v2
    #[serde(default)]
    pub leaf_drop: Option<LeafDropDef>,     // v2
}

pub struct FoliageClustersDef {
    pub count: u32,
    pub emitter_radius: f32,
    pub leaves_per_cluster: u32,
    pub leaf_scale_min: f32,
    pub leaf_scale_max: f32,
}

pub struct FoliageMaterialDef {
    pub leaf_texture: String,
    pub color_highlight: [f32; 3],
    pub color_midtone: [f32; 3],
    pub color_shadow: [f32; 3],
    pub toon_bands: u8,       // validated: must be 2, 3, or 4
    pub ao_intensity: f32,
}

pub struct FoliageWindDef {   // v2
    pub macro_strength: f32,
    pub macro_frequency: f32,
    pub micro_strength: f32,
    pub micro_frequency: f32,
}

pub struct LeafDropDef {      // v2
    pub rate: f32,
    pub lifetime: f32,
    pub drift_strength: f32,
}
```

### `PrefabDef` addition

```rust
#[serde(default)]
pub foliage: Option<FoliageDef>,
```

Validation rules added to `PrefabCatalog::validate()`:
- `kind == Foliage` and `foliage.is_none()` → error: "Foliage prefab requires a `foliage` block".
- `toon_bands` not in `[2, 3, 4]` → error.
- `leaves_per_cluster` == 0 → error.

---

## Runtime changes

### New capability (`capabilities/foliage.rs`)

**`foliage_spawn_system`** — runs once on `SceneEvent::Ready`.

For each entity whose prefab has `kind: Foliage`:

1. **Trunk** — if `trunk` is `Some(key)`, resolve the catalog key and spawn a GLB child entity (same path as `Prop` spawning in `entity_spawner.rs`).

2. **Clusters** — for each cluster `0..count`:
   - Sample a random position on a sphere of `emitter_radius` using Fibonacci sphere distribution for uniform coverage.
   - Call `build_cluster_mesh(cluster_center, def)` → `Mesh`:
     - For each leaf `0..leaves_per_cluster`:
       - Sample a random position on a smaller sphere around the cluster centre.
       - Build a flat quad at that position (4 vertices, 6 indices).
       - Set vertex attribute `LEAF_CENTER` = leaf position (used by the billboard shader).
       - Set vertex normal = `normalize(leaf_pos - cluster_center)` (sphere normal for toon shading).
       - Apply random scale in `[leaf_scale_min, leaf_scale_max]`.
     - Return the combined mesh (all leaf quads packed, `count × leaves_per_cluster` quads total).
   - Spawn one entity with the combined cluster mesh + `FoliageMaterial` handle.

All cluster entities are spawned as children of the foliage root entity, which carries the trunk's transform.

**Mesh attribute layout** (per vertex in a leaf card quad):

| Attribute | Type | Content |
|---|---|---|
| `Mesh::ATTRIBUTE_POSITION` | `Vec3` | corner offset from leaf centre in local space |
| `Mesh::ATTRIBUTE_NORMAL` | `Vec3` | sphere normal (leaf centre → cluster centre) |
| `Mesh::ATTRIBUTE_UV_0` | `Vec2` | quad UVs (0–1 per card) |
| `ATTRIBUTE_LEAF_CENTER` | `Vec3` | leaf anchor point in cluster-local space |

### `FoliageMaterial` (`capabilities/foliage.rs`)

Implements Bevy's `Material` trait with a custom `foliage.wgsl` shader (embedded via `include_str!`).

**Vertex shader** (key logic):

```wgsl
@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    // 1. Compute leaf centre in clip space (no billboard, just translation)
    let leaf_world = mesh.model * vec4(in.leaf_center, 1.0);
    let leaf_clip  = view.view_proj * leaf_world;

    // 2. Expand the quad corner in view space using camera right/up vectors
    //    (extracted from view matrix columns — strips all rotation from the model)
    let right = vec3(view.view[0][0], view.view[1][0], view.view[2][0]);
    let up    = vec3(view.view[0][1], view.view[1][1], view.view[2][1]);
    let offset = in.position.x * right + in.position.y * up;

    out.position = leaf_clip + view.projection * vec4(offset, 0.0, 0.0);
    out.world_normal = (mesh.model * vec4(in.normal, 0.0)).xyz;  // sphere normal
    out.uv = in.uv;
}
```

**Fragment shader** (key logic):

```wgsl
@fragment
fn fragment(in: FragmentOutput) -> @location(0) vec4<f32> {
    // Alpha clip — hard discard for painterly silhouette, no blending
    let alpha = textureSample(leaf_texture, leaf_sampler, in.uv).a;
    if alpha < 0.5 { discard; }

    // Toon cel shading using sphere normal
    let NdotL = dot(normalize(in.world_normal), normalize(material.sun_direction));
    let tone  = select_tone(NdotL, material.toon_bands);  // returns 0.0, 0.6, or 1.0

    // AO: darken based on how far the normal points away from the sun hemisphere
    let ao = 1.0 - material.ao_intensity * max(0.0, -NdotL);

    let base_color = mix(
        mix(material.color_shadow, material.color_midtone, tone),
        material.color_highlight, max(0.0, tone - 0.9)
    );
    return vec4(base_color * ao, 1.0);
}
```

**Material bindings:**

```rust
#[derive(Asset, TypePath, AsBindGroup)]
pub struct FoliageMaterial {
    #[texture(0)]  #[sampler(1)]
    pub leaf_texture: Handle<Image>,
    #[uniform(2)]
    pub params: FoliageMaterialParams,  // colors, toon_bands, ao_intensity, sun_direction
}
```

`sun_direction` is updated each frame from the scene's directional light direction by `foliage_lighting_sync_system`.

---

## Relationship to other features

- **Toon / cel shading** (`toon_shading.md`) — that feature applies cel shading to existing GLB models via `CustomMaterial`. `FoliageMaterial` is a separate, standalone shader; the two coexist independently.
- **Consistent RON enum casing** (`consistent_ron_enum_casing.md`) — `PrefabKind::Foliage` is added in the same migration commit or immediately after.
- **Deferred rendering** — `FoliageMaterial` implements `Material` without overriding `opaque_render_method()`; it defaults to `Forward`, same as other custom materials. Deferred/forward mix is safe.
- **Particle system** — leaf drop (v2) spawns particles from cluster positions using the existing particle spawner.

---

## v1 scope

- `kind: Foliage` prefab type fully supported
- Trunk GLB + procedural leaf card clusters
- `FoliageMaterial`: billboard + sphere normals + alpha clip + 3-tone toon shading
- AO intensity parameter
- No wind, no leaf drop

## v2 scope

- GPU wind: macro sway (whole cluster) + micro flutter (per-leaf via vertex ID) in vertex shader
- Particle leaf drop via existing particle system
- `SetFoliageWind(strength: f32)` action for scripted events (storm, calm)

---

## Files to create / modify

- `crates/ironhold_core/src/capabilities/foliage.rs` (new)
- `crates/ironhold_core/src/assets/foliage.wgsl` (new, embedded in foliage.rs)
- `crates/ironhold_core/src/capabilities/mod.rs` — register capability
- `crates/ironhold_core/src/schema/catalog.rs` — `FoliageDef` and related structs; `PrefabKind::Foliage`
- `crates/ironhold_core/src/runtime/scene_manager/entity_spawner.rs` — `Foliage` arm in spawn match
- `assets/projects/particles_demo/` or a new `foliage_demo/` — example project

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved (see below)
- [ ] `PrefabKind::Foliage` added (coordinate with enum casing migration)
- [ ] `FoliageDef`, `FoliageClustersDef`, `FoliageMaterialDef` structs in `schema/catalog.rs`
- [ ] `PrefabCatalog::validate()` — Foliage-specific validation rules
- [ ] `ATTRIBUTE_LEAF_CENTER` custom mesh attribute defined
- [ ] `build_cluster_mesh()` — Fibonacci sphere sampling, leaf quad packing, sphere normal baking
- [ ] `FoliageMaterial` struct + `AsBindGroup` derive
- [ ] `foliage.wgsl` — billboard vertex shader + toon fragment shader
- [ ] `foliage_spawn_system` — reads `FoliageDef`, builds cluster meshes, spawns entities
- [ ] `foliage_lighting_sync_system` — updates `sun_direction` uniform each frame from directional light
- [ ] Registered in `capabilities/mod.rs` plugin
- [ ] Entity spawner `Foliage` arm in `entity_spawner.rs`
- [ ] Example prefab in an existing or new demo project with a leaf brush-stroke texture
- [ ] `cargo test -p ironhold_core` passes
- [ ] `python test_web.py --skip-build` baseline passes (or new baselines committed)
- [ ] Docs: `crates/ironhold_core/src/CLAUDE.md` — note FoliageMaterial pattern

---

## Pre-implementation checklist

- [ ] **Fibonacci sphere vs random sampling for cluster placement.** Fibonacci gives perfectly uniform distribution without clustering artefacts. Recommended for cluster centres (`count` clusters). For leaf positions within a cluster, random distribution is more organic. Use both: Fibonacci for clusters, random for leaves.
- [ ] **`toon_bands` as a uniform vs shader variant.** A `u32` uniform with a runtime `select` in WGSL adds one branch per fragment. Alternatively, generate three shader variants at compile time. For v1, the uniform branch is fine — profiling can guide a later switch.
- [ ] **Leaf texture asset type.** The `leaf_texture` key resolves through `LoadedAssetCatalog` to a `Handle<Image>` the same way particle textures do. Confirm the asset loader path for textures is already in place — it is, via `AssetServer` in the particle system.
- [ ] **Sun direction sync strategy.** `foliage_lighting_sync_system` queries the scene's `DirectionalLight` entity for its `GlobalTransform` to extract the light direction. Runs in `PostUpdate` after the transform propagation stage. If no directional light exists in the scene, default to `Vec3::NEG_Y` (directly overhead).
- [ ] **Available assets.** The following shared assets are ready to use immediately:
  - Trunk model: `assets/shared/models/plants/trunk_with_branches_01.glb` → catalog key `"models/plants/trunk_with_branches_01"`. No preview image yet — generate one with `python tools/glb_preview/preview.py assets/shared/models/plants/ --avif`.
  - Leaf texture: `assets/shared/textures/foliage/leaf_brush_01.png` → catalog key `"textures/foliage/leaf_brush_01"`.

- [ ] **New demo project vs existing project.** Adding a `foliage_demo` project is the cleanest option — it avoids cluttering existing projects and gives designers a clear reference. Requires the three registration steps in `CLAUDE.md` (test_web.py, baseline screenshot, index.html card).

---

## Acceptance criteria

- Given a `kind: Foliage` prefab placed in a scene, the tree renders with visible leaf clusters and correct toon shading at scene load.
- Given the camera rotating around the tree, leaf cards always face the camera with no geometry tearing.
- Given a directional light at a 45° angle, exactly two distinct light bands (highlight and shadow) are visible when `toon_bands: 2`; three when `toon_bands: 3`.
- Given `ao_intensity: 0.5`, the underside of foliage clusters is visibly darker than the top.
- Given `trunk: Some("models/plants/trunk_with_branches_01")`, the trunk GLB renders at the prefab position as a child of the foliage root.
- Given `trunk: None`, only leaf clusters render (valid for bushes).
- Given `python test_web.py --skip-build`, the foliage demo project passes smoke and baseline tests.
- Given `python test_web.py --webgpu --skip-build --project foliage_demo`, smoke test passes (WebGPU path renders correctly).
- `ironhold_cli validate assets/projects/foliage_demo` exits 0.
- A `kind: Foliage` prefab with no `foliage` block fails `validate()` with a clear error.

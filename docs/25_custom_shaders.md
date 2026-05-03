# Custom Shaders (WGSL)

> **Doc type:** Reference + Guide
>
> **Status legend:**
> - ✅ **Implemented** — exists in code today
> - 🧪 **Prototype / Partial** — exists but incomplete or unstable
> - 🧭 **Planned** — intended design; not implemented yet

---

## Philosophy

WGSL is the first-class shader language for this engine. It is the native language of WebGPU, which means:

- **No transpilation overhead.** WGSL runs natively in the browser (WebGPU) and on desktop (Bevy's wgpu backend). No GLSL or HLSL conversion step.
- **Identical output on all platforms.** The same `.wgsl` file produces the same pixels on desktop and in the browser. This enforces the project's cross-platform consistency principle.
- **No LUT dependencies.** Shaders that require a look-up-table texture (e.g., `TonyMcMapface`, `BlenderFilmic` tonemapping) are excluded from the engine because LUT bandwidth reduces performance and creates per-platform differences. Custom shaders must follow the same constraint.
- **Data-driven authoring.** Shaders are loaded from disk as assets. A designer can create a new visual effect by writing a `.wgsl` file and wiring it up in RON — no engine recompile needed.

---

## Where shaders live ✅

```
assets/
  shared/shaders/       ← built-in / shared shaders; usable by any project
  projects/{name}/shaders/   ← project-specific shaders (convention, not enforced)
```

Shared shaders are prefixed `custom_` by convention (e.g. `custom_pbr.wgsl`, `custom_fresnel.wgsl`).

---

## How WGSL shaders plug in ✅

The engine exposes one WGSL extension point today: **`CustomMaterial`** (fragment shader only).

```
RON assets.ron                     WGSL shader file
  materials: {                       @fragment
    "my_mat": (                      fn fragment(in: VertexOutput) → @location(0) vec4<f32> {
      kind: Custom((                   let c = material.params_0;
        shader: "shaders/my.wgsl",     return vec4<f32>(c.r, c.g, c.b, 1.0);
        colors: { "tint": (...) },   }
        floats: { "power": 2.0 },
      )),
    ),
  }
        ↓
  CustomMaterial (Bevy Material asset)
  ↓ specialize()
  GPU pipeline (one pipeline per unique shader handle)
```

The engine reads the RON definition, packs uniforms, loads the shader from the asset server, and creates a `CustomMaterial` that Bevy's render pipeline picks up automatically.

---

## WGSL binding contract ✅

Every custom fragment shader **must** declare these exact bindings. Copy this header verbatim:

```wgsl
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var texture_0: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var sampler_0: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var texture_1: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var sampler_1: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var texture_2: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var sampler_2: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var texture_3: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var sampler_3: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // your code here
    return vec4<f32>(1.0);
}
```

**Rules:**
- All 8 texture/sampler bindings must be declared, even if unused. Unused slots receive a 1×1 white fallback.
- `#{MATERIAL_BIND_GROUP}` is a Bevy shader import macro — do not replace it with a literal group number.
- The fragment entry point must be named `fragment` and return `@location(0) vec4<f32>`.
- **WebGPU strictly validates binding interfaces.** Missing or mismatched bindings will produce a GPU error, not a Rust panic.

---

## Uniform packing ✅

The 4×Vec4 uniform block is filled from the RON `colors` and `floats` maps.

**Packing order** (both maps sorted alphabetically by key):
1. Each `colors` entry occupies one full `vec4<f32>` slot.
2. `floats` entries are packed 4-per-`vec4` into the remaining slots.

```text
Example: 1 color ("base_color") + 3 floats ("metallic", "roughness", "tiling")
  params_0 = base_color.rgba         ← 1st color (alphabetically)
  params_1 = (metallic, roughness, tiling, 0.0)   ← floats packed together
  params_2 = (0, 0, 0, 0)
  params_3 = (0, 0, 0, 0)

Example: 2 colors ("color_a", "color_b") + 4 floats ("f0", "f1", "f2", "f3")
  params_0 = color_a.rgba
  params_1 = color_b.rgba
  params_2 = (f0, f1, f2, f3)
  params_3 = (0, 0, 0, 0)
```

**Capacity:** 4 colors maximum, or 16 floats maximum, or any combination that fits in 4×Vec4 total.

In your WGSL shader, reference the packed slot by position — **the RON key names are not visible in the shader**:

```wgsl
let base_color = material.params_0;        // first color (alphabetically "base_color")
let metallic   = material.params_1.x;      // first float (alphabetically "metallic")
let roughness  = material.params_1.y;      // second float
let tiling     = material.params_1.z;      // third float
```

Always document which slot maps to which uniform in a comment at the top of your shader. See `custom_pbr.wgsl` for an example.

---

## Texture slots ✅

Up to 4 textures (`texture_0` … `texture_3`) can be assigned per material. Declare them in the RON `textures` map using the exact slot name as the key:

```ron
textures: {
    "texture_0": "shared/textures/my_albedo.png",
    "texture_2": "shared/textures/my_normal.png",
    // texture_1 and texture_3 are unused — they get a 1×1 white fallback
}
```

Sample in WGSL as usual:

```wgsl
let albedo = textureSample(texture_0, sampler_0, in.uv);
```

---

## Available shared shaders ✅

All shaders live in `assets/shared/shaders/`.

| File | Effect | Key uniforms |
|------|--------|-------------|
| `custom_unlit_color.wgsl` | Solid unlit colour | `params_0` = RGBA colour |
| `custom_pbr.wgsl` | Full Bevy PBR pipeline | `params_0` = base_color; `params_1.x` = metallic; `params_1.y` = roughness |
| `custom_checker_uv.wgsl` | Procedural UV-space checkerboard — clean on cuboids/planes; seam visible on spheres | `params_0` = color_a; `params_1` = color_b; `params_2.x` = tiling (UV repetitions) |
| `custom_checker_world.wgsl` | Procedural 3-D world-space checkerboard — seamless on any geometry; pattern shared across the world grid | `params_0` = color_a; `params_1` = color_b; `params_2.x` = tiling (cells/world-unit) |
| `custom_checker_local.wgsl` | Procedural 3-D local (object-space) checkerboard — seamless, pattern centered on each object independently | `params_0` = color_a; `params_1` = color_b; `params_2.x` = tiling (cells/local-unit) |
| `custom_gradient.wgsl` | UV V-axis gradient (bottom→top); correct on upright meshes | `params_0` = bottom_color; `params_1` = top_color |
| `custom_fresnel.wgsl` | Fresnel rim — true N·V dot product; rim sits at silhouette edges regardless of camera angle | `params_0` = color_a (rim); `params_1` = color_b (face); `params_2.x` = rim_power (2=wide, 5=tight) |
| `custom_world_stripes.wgsl` | World-space horizontal stripes | `params_0` = color_a; `params_1` = color_b; `params_2.x` = frequency |
| `custom_normal_vis.wgsl` | World-space normal visualisation | No uniforms |
| `custom_emissive_fresnel.wgsl` | Transparent emissive fresnel (glass-orb or energy-field) | `params_0` = base_color; `params_1` = rim_color; `params_2.x` = rim_power |
| `custom_pulse.wgsl` | Pulsing emissive — breathes in/out over time | `params_0` = color; `params_1.x` = min_alpha; `params_1.y` = speed (pulses/sec) |
| `custom_texture_tint.wgsl` | Texture with a colour tint multiplied on top (UV-based; avoid on spheres/cylinders) | `texture_0` = albedo texture; `params_0` = tint (r,g,b,a) |
| `custom_texture_triplanar.wgsl` | Triplanar world-space texture — no UV seams on any geometry | `texture_0` = texture; `params_0` = tint; `params_1.x` = blend_sharpness (4=soft, 8=sharp); `params_1.y` = tiling (repeats/unit) |
| `custom_dissolve_uv.wgsl` | Procedural noise cutout with glowing edge — UV-space; clean on flat/box geometry | `params_0` = base_color; `params_1` = edge_color; `params_2.x` = edge_width; `params_2.y` = threshold; `params_2.z` = tiling (UV scale) |
| `custom_dissolve_world.wgsl` | Procedural noise cutout with glowing edge — world-space XZ; seamless on any geometry | `params_0` = base_color; `params_1` = edge_color; `params_2.x` = edge_width; `params_2.y` = threshold; `params_2.z` = tiling (world-units) |
| `custom_dissolve_local.wgsl` | Procedural noise cutout with glowing edge — local (object-space) XZ; seamless, pattern centred on each object | `params_0` = base_color; `params_1` = edge_color; `params_2.x` = edge_width; `params_2.y` = threshold; `params_2.z` = tiling (local-units) |
| `custom_material_default.wgsl` | **Fallback** — magenta | None (compiled in; do not reference in RON) |

`terrain.wgsl` is used exclusively by `TerrainMaterial` and is not available for custom materials.

---

## Writing a new shader — step by step ✅

1. **Create the `.wgsl` file** in `assets/shared/shaders/` (shared) or `assets/projects/{name}/shaders/` (project-specific).
2. **Copy the binding header** from the [contract section](#wgsl-binding-contract-) above.
3. **Document your slot mapping** at the top of the shader (which `params_*` field maps to which uniform).
4. **Write your `fragment` function.**
5. **Declare the material in `assets.ron`**:
   ```ron
   "my_new_mat": (
     kind: Custom((
       shader: "shared/shaders/my_new.wgsl",
       colors:  { "tint": (r: 1.0, g: 0.5, b: 0.2, a: 1.0) },
       floats:  { "power": 3.0 },
     )),
   ),
   ```
6. **Reference it in a prefab** via `material: "my_new_mat"`.
7. **Test in a web build** — WebGPU is stricter than native wgpu. Run `python test_web.py` or at minimum load the project in a browser.

---

## Debugging tips ✅

| Symptom | Cause | Fix |
|---------|-------|-----|
| Entity renders magenta | Shader path missing, wrong, or asset not yet loaded | Check path in RON; verify file exists in `assets/` |
| GPU error / black screen in browser | Binding mismatch — WebGPU validates binding interfaces strictly | Ensure all 8 texture/sampler bindings are declared |
| Floats read as 0 | Wrong `params_*` slot — check alphabetical sort order of your keys | Re-derive packing order (sort colors first, then floats, both A→Z) |
| Colours look wrong | Linear vs sRGB mismatch | RON colors are linear sRGB; call `pow(c, vec4(2.2))` if your shader expects gamma |
| Crash on WASM with `BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED` | Custom struct added to uniform block — breaks 16-byte WebGPU requirement | Keep uniforms as `vec4<f32>`; pad if needed |

---

## Emissive and glow effects ✅

HDR camera mode and bloom are **not enabled** in this engine (see [Rendering philosophy in `docs/20_data_formats.md`](20_data_formats.md#rendering-philosophy)). This has one important consequence for shader authors: **fragment output values above 1.0 clip — they do not produce a bloom halo.**

To achieve a self-illuminated or glowing look without HDR:

- Set `unlit: true` on the material. This bypasses the lighting pipeline entirely; the shader output is the final pixel colour. **`unlit: true` also automatically adds `NotShadowCaster` to the entity** — an object that ignores lighting has no business casting shadows.
- Use `alpha_mode: Add` for additive blending. Dark parts of the surface become transparent; bright parts add on top of what is behind them. This is the standard approach for energy fields, particle-like meshes, and holographic overlays.
- Use `alpha_mode: Blend` when you need a solid-but-translucent surface (e.g. a glass shield) rather than a purely additive glow.

**Minimal emissive material in RON:**
```ron
"mat_glow": (
  kind: Custom((
    shader: "shared/shaders/custom_unlit_color.wgsl",
    colors: { "color": (r: 0.2, g: 0.8, b: 1.0, a: 0.7) },
  )),
  alpha_mode: Add,
  unlit: true,
),
```

Keep colour values in the `[0, 1]` range. Values above 1.0 offer no visual benefit without HDR and will simply clip to white.

---

## Double-sided and interior rendering ✅

By default, meshes are rendered with back-face culling enabled — fragments on the inside surface of a mesh are discarded. Set `double_sided: true` to disable culling and render both faces:

```ron
"mat_sky": (
  kind: Custom((
    shader: "shared/shaders/custom_gradient.wgsl",
    colors: {
      "color_a": (r: 0.52, g: 0.70, b: 0.88, a: 1.0),  // horizon
      "color_b": (r: 0.03, g: 0.10, b: 0.48, a: 1.0),  // zenith
    },
  )),
  double_sided: true,
  unlit: true,
),
```

**Sky sphere pattern:** Spawn a large sphere (radius ~500) at the scene centre and apply a `double_sided: true, unlit: true` gradient material. The camera is always inside the sphere, so it sees the inner faces. Because `unlit: true` also suppresses shadow casting, the sky sphere does not project a shadow onto the scene.

```ron
// prefabs.ron
"sky_sphere": (
  kind: "primitive",
  model: "Sphere",
  material: "mat_sky",
  components: (),
  primitive: (radius: 500.0),
),
```

**Implementation note:** `double_sided: true` creates a distinct GPU pipeline (separate `CustomMaterialKey`) from the default single-sided pipeline, so mixing both variants in a scene has no overhead beyond the extra pipeline compile at startup.

**Normal flipping — per-shader compatibility:** `double_sided` works at the rasterizer stage, not the shader stage. When a back face is rendered, `in.world_normal` still points in the same direction as the front face — WGSL does not automatically flip normals for back faces. Shaders that do not use normals are fully compatible; shaders that depend on normals will produce incorrect results when viewed from the inside:

| Shader | Safe with `double_sided`? | Reason |
|--------|--------------------------|--------|
| `custom_gradient.wgsl` | ✅ | UV-only — normals unused |
| `custom_checker_*.wgsl` | ✅ | Position/UV-only |
| `custom_texture_*.wgsl` | ✅ | UV/position-only |
| `custom_unlit_color.wgsl` | ✅ | No normals |
| `custom_dissolve_*.wgsl` | ✅ | Noise + position-only |
| `custom_fresnel.wgsl` | ❌ | `dot(N, V)` is negative for back faces — rim migrates from silhouette to face centre |
| `custom_emissive_fresnel.wgsl` | ❌ | Same N·V dependency |
| `custom_pbr.wgsl` | ❌ | Full lighting breaks with inverted normals |
| `custom_normal_vis.wgsl` | ❌ | Displays outward normals regardless of face |

If you need a normal-dependent shader on a double-sided surface, read `@builtin(front_facing)` in WGSL and negate the normal for back faces:

```wgsl
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> @location(0) vec4<f32> {
    let n = normalize(select(-in.world_normal, in.world_normal, is_front));
    // use n instead of in.world_normal
}
```

**Other use cases:** double-sided leaves/grass billboards, two-sided decals, portal surfaces, thin cloth.

---

## Current limitations 🧭

### Fragment-only ✅ / Vertex shader 🧭
`CustomMaterial::specialize()` only swaps in the user shader for the **forward fragment pass**. The vertex shader and prepass shaders remain fixed.

This means you cannot currently:
- Displace vertices (e.g., animated grass, water waves, morphing)
- Generate custom UVs in the vertex stage
- Write full geometry-altering effects

Vertex shader override is planned. Until it lands, vertex-level effects must be done CPU-side.

### Fixed uniform capacity ✅ (64 bytes)
4×Vec4 is sufficient for most simple-to-medium effects. Effects requiring many parameters (e.g., a full atmosphere shader) will eventually need an expanded uniform block or a storage buffer.

### Compute shaders 🧭
Terrain mesh generation currently runs on Bevy's `AsyncComputeTaskPool` (CPU threads). Moving heavy workloads to GPU compute shaders would provide a large performance gain and is a natural next use of WGSL. No compute shader infrastructure exists yet — this is the next planned WGSL expansion.

---

## WebGPU alignment rules ✅

Any uniform struct bound via `#[uniform(...)]` must obey **16-byte alignment**:

- `Vec4` = 16 bytes ✅
- `Vec3` = 12 bytes ❌ — pad to `Vec4`
- `f32` = 4 bytes ❌ — group 4 into a `Vec4` or pad to `vec4<f32>` in WGSL
- Custom structs — total size must be a multiple of 16 bytes

`CustomMaterialUniforms` (4×Vec4 = 64 bytes) and `TerrainMaterial.uv_scale` (Vec4 with `.yzw` padding) both comply. Violating this causes a `BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED` panic in WASM builds.

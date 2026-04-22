#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::pbr_fragment::pbr_input_from_vertex_output
#import bevy_pbr::pbr_functions::apply_pbr_lighting

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var texture_0: texture_2d<f32>; // basecolor
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var sampler_0: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var texture_1: texture_2d<f32>; // normal
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var sampler_1: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var texture_2: texture_2d<f32>; // roughness (R channel)
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var sampler_2: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var texture_3: texture_2d<f32>; // ambient occlusion (R channel)
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var sampler_3: sampler;

// Full PBR triplanar shader — basecolor, normal, roughness, and AO, all
// projected from three world-space axis planes and blended by surface normal.
//
// Triplanar projection eliminates UV seams and pole pinch on spheres,
// cylinders, and organic shapes by driving UVs from world position rather
// than precomputed mesh UVs.
//
// Normal reorientation uses the Whiteout blend (Ben Golus formulation):
// each projection plane's tangent-space normal is swizzled into world space
// before blending, so the resulting normal sits on the correct side of the
// surface for every axis.
//
// ── Uniform layout ───────────────────────────────────────────────────────────
// (colors first, then floats — both groups sorted alphabetically by key)
//
//   colors (1 entry):
//     "tint"            → params_0       (r,g,b,a) albedo multiplier; (1,1,1,1) = no tint
//   floats (4 entries packed into params_1):
//     "blend_sharpness" → params_1.x     axis-blend transition (2=soft, 4=natural, 8=sharp)
//     "metallic"        → params_1.y     0 = dielectric, 1 = full metal
//     "normal_strength" → params_1.z     0 = disabled, 1 = map as-is, 2 = exaggerated
//     "tiling"          → params_1.w     repeats per world unit (0.05=large slab, 1.0=1/unit)
//
// ── Partial-texture contract ──────────────────────────────────────────────────
// All four texture slots are always bound. Unused slots receive a 1×1 white
// fallback from the engine. The table below shows the safe behaviour for each
// omitted map:
//
//   texture_0 (basecolor) — required; white fallback renders as fully white.
//   texture_1 (normal)    — set normal_strength: 0.0 to disable.
//                           White fallback + strength > 0 produces wrong normals.
//                           The Whiteout formula collapses to the geometric
//                           normal exactly when strength = 0.0, regardless of
//                           what the texture contains.
//   texture_2 (roughness) — omit freely; white fallback → roughness = 1.0 (fully
//                           rough). Acceptable for most stone/wood/organic surfaces.
//   texture_3 (AO)        — omit freely; white fallback → diffuse_occlusion = 1.0
//                           (no occlusion), which is the correct neutral default.
//
// ── RON example (full set) ───────────────────────────────────────────────────
//   "mat_stone": (
//     kind: Custom((
//       shader: Some("shared/shaders/custom_texture_triplanar_pbr.wgsl"),
//       textures: {
//         "texture_0": "shared/textures/MySet/basecolor.png",
//         "texture_1": "shared/textures/MySet/normal.png",
//         "texture_2": "shared/textures/MySet/roughness.png",
//         "texture_3": "shared/textures/MySet/ambientOcclusion.png",
//       },
//       colors:  { "tint": (r:1.0, g:1.0, b:1.0, a:1.0) },
//       floats:  { "blend_sharpness": 4.0, "metallic": 0.0, "normal_strength": 1.0, "tiling": 0.05 },
//     )),
//   ),
//
// ── RON example (no normal map) ──────────────────────────────────────────────
//   "mat_stone_flat": (
//     kind: Custom((
//       shader: Some("shared/shaders/custom_texture_triplanar_pbr.wgsl"),
//       textures: {
//         "texture_0": "shared/textures/MySet/basecolor.png",
//         "texture_2": "shared/textures/MySet/roughness.png",
//       },
//       colors:  { "tint": (r:1.0, g:1.0, b:1.0, a:1.0) },
//       floats:  { "blend_sharpness": 4.0, "metallic": 0.0, "normal_strength": 0.0, "tiling": 0.05 },
//     )),
//   ),

fn triplanar_weights(world_normal: vec3<f32>, blend_sharpness: f32) -> vec3<f32> {
    let w = pow(abs(world_normal), vec3<f32>(blend_sharpness));
    return w / (w.x + w.y + w.z);
}

fn triplanar_rgba(
    tex: texture_2d<f32>, samp: sampler,
    pos: vec3<f32>, weights: vec3<f32>,
) -> vec4<f32> {
    let uv_yz = pos.yz;
    let uv_xz = pos.xz;
    let uv_xy = pos.xy;
    let cx = textureSampleGrad(tex, samp, fract(uv_yz), dpdx(uv_yz), dpdy(uv_yz));
    let cy = textureSampleGrad(tex, samp, fract(uv_xz), dpdx(uv_xz), dpdy(uv_xz));
    let cz = textureSampleGrad(tex, samp, fract(uv_xy), dpdx(uv_xy), dpdy(uv_xy));
    return cx * weights.x + cy * weights.y + cz * weights.z;
}

fn triplanar_r(
    tex: texture_2d<f32>, samp: sampler,
    pos: vec3<f32>, weights: vec3<f32>,
) -> f32 {
    let uv_yz = pos.yz;
    let uv_xz = pos.xz;
    let uv_xy = pos.xy;
    let sx = textureSampleGrad(tex, samp, fract(uv_yz), dpdx(uv_yz), dpdy(uv_yz)).r;
    let sy = textureSampleGrad(tex, samp, fract(uv_xz), dpdx(uv_xz), dpdy(uv_xz)).r;
    let sz = textureSampleGrad(tex, samp, fract(uv_xy), dpdx(uv_xy), dpdy(uv_xy)).r;
    return sx * weights.x + sy * weights.y + sz * weights.z;
}

fn triplanar_normal_world(
    tex: texture_2d<f32>, samp: sampler,
    pos: vec3<f32>, world_normal: vec3<f32>, weights: vec3<f32>, strength: f32,
) -> vec3<f32> {
    let uv_yz = pos.yz;
    let uv_xz = pos.xz;
    let uv_xy = pos.xy;

    // Unpack normal map samples from [0,1] to [-1,1]
    var raw_x = textureSampleGrad(tex, samp, fract(uv_yz), dpdx(uv_yz), dpdy(uv_yz)).xyz * 2.0 - 1.0;
    var raw_y = textureSampleGrad(tex, samp, fract(uv_xz), dpdx(uv_xz), dpdy(uv_xz)).xyz * 2.0 - 1.0;
    var raw_z = textureSampleGrad(tex, samp, fract(uv_xy), dpdx(uv_xy), dpdy(uv_xy)).xyz * 2.0 - 1.0;

    // Scale tangent-plane components (XY) by strength; leave depth (Z) untouched.
    raw_x = vec3<f32>(raw_x.xy * strength, raw_x.z);
    raw_y = vec3<f32>(raw_y.xy * strength, raw_y.z);
    raw_z = vec3<f32>(raw_z.xy * strength, raw_z.z);

    let n = world_normal;

    // Whiteout blend: inject the surface normal's contribution into each
    // projection's tangent XY before swizzling to world space.
    // Formulation: Ben Golus, "Normal Mapping for a Triplanar Shader" (2017).
    let nx = vec3<f32>(raw_x.xy + n.zy, raw_x.z * n.x);
    let ny = vec3<f32>(raw_y.xy + n.xz, raw_y.z * n.y);
    let nz = vec3<f32>(raw_z.xy + n.xy, raw_z.z * n.z);

    // Swizzle each result back to world space (different for each projection axis)
    // and blend by the triplanar weights.
    return normalize(nx.zyx * weights.x + ny.xzy * weights.y + nz.xyz * weights.z);
}

@fragment
fn fragment(
    mesh: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {
    let tint            = material.params_0;
    let blend_sharpness = material.params_1.x;
    let metallic        = material.params_1.y;
    let normal_strength = material.params_1.z;
    let tiling          = material.params_1.w;

    let pos     = mesh.world_position.xyz * tiling;
    let geo_n   = normalize(mesh.world_normal);
    let weights = triplanar_weights(geo_n, blend_sharpness);

    let base_color = triplanar_rgba(texture_0, sampler_0, pos, weights) * tint;
    let roughness  = triplanar_r(texture_2, sampler_2, pos, weights);
    let ao         = triplanar_r(texture_3, sampler_3, pos, weights);
    let world_n    = triplanar_normal_world(texture_1, sampler_1, pos, geo_n, weights, normal_strength);

    var pbr = pbr_input_from_vertex_output(mesh, is_front, false);
    pbr.material.base_color           = base_color;
    pbr.material.metallic             = metallic;
    pbr.material.perceptual_roughness = roughness;
    pbr.N                             = world_n;
    pbr.diffuse_occlusion             = vec3<f32>(ao);
    return apply_pbr_lighting(pbr);
}

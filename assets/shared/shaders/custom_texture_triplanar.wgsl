#import bevy_pbr::forward_io::VertexOutput

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var texture_0: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var sampler_0: sampler;

// Triplanar texture projection — no UV seams on any geometry.
//
// Projects texture_0 from the world-space X, Y, and Z axis planes simultaneously
// and blends the three samples by how much the surface normal faces each axis.
// Because world position drives the UV coordinates rather than the mesh's
// precomputed UV attribute, there is no meridian seam or pole pinch — spheres,
// cylinders, and organic shapes all tile cleanly.
//
// fract() manually wraps UVs into [0,1) before sampling so the result is correct
// regardless of the sampler's address mode (Bevy defaults to ClampToEdge).
// textureSampleGrad with pre-fract derivatives keeps the mip level smooth across
// the wrap boundary (avoids the blur artifact that plain fract() + textureSample
// would produce at the seam where 0.9999 wraps back to 0.0000).
//
// Uniform layout:
//   colors (1 entry):
//     "tint"            → params_0  (r,g,b,a) — multiplied over the final blend;
//                         use (1,1,1,1) for no tint
//   floats (2 entries, alphabetical):
//     "blend_sharpness" → params_1.x — transition width between projection axes
//                         (2 = very soft, 4 = natural default, 8 = sharp/architectural)
//     "tiling"          → params_1.y — texture repeats per world unit
//                         (0.2 = large tiles, 1.0 = one tile per unit, 2.0 = small tiles)
//
// RON example:
//   "mat_stone": (
//     kind: Custom((
//       shader: Some("shared/shaders/custom_texture_triplanar.wgsl"),
//       textures: { "texture_0": "shared/textures/Stylized_Bricks_004_SD/Stylized_Bricks_004_basecolor.png" },
//       colors:  { "tint": (r:1.0, g:1.0, b:1.0, a:1.0) },
//       floats:  { "blend_sharpness": 4.0, "tiling": 0.5 },
//     )),
//   ),
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tint            = material.params_0;
    let blend_sharpness = material.params_1.x;
    let tiling          = material.params_1.y;

    let pos  = in.world_position.xyz * tiling;
    let norm = abs(in.world_normal);

    // Raise each normal component to blend_sharpness so the transition between
    // projection axes is narrow (high) or wide (low). Normalise so weights sum to 1.
    let w       = pow(norm, vec3<f32>(blend_sharpness));
    let weights = w / (w.x + w.y + w.z);

    // UV coordinates for each axis plane (pre-fract, used for derivative calculation).
    let uv_yz = pos.yz;
    let uv_xz = pos.xz;
    let uv_xy = pos.xy;

    // fract() wraps into [0,1) so large world coordinates sample correctly even with
    // a clamp-to-edge sampler. textureSampleGrad with the pre-fract derivatives
    // prevents the GPU from treating the 0.9999→0.0000 discontinuity as a huge
    // derivative and selecting the wrong (blurry) mip level.
    let col_x = textureSampleGrad(texture_0, sampler_0, fract(uv_yz), dpdx(uv_yz), dpdy(uv_yz));
    let col_y = textureSampleGrad(texture_0, sampler_0, fract(uv_xz), dpdx(uv_xz), dpdy(uv_xz));
    let col_z = textureSampleGrad(texture_0, sampler_0, fract(uv_xy), dpdx(uv_xy), dpdy(uv_xy));

    let color = col_x * weights.x + col_y * weights.y + col_z * weights.z;
    return color * tint;
}

#import bevy_pbr::forward_io::VertexOutput

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// Procedural 3-D world-space checkerboard.
//
// Uses world position rather than UV so the pattern is seamless on spheres,
// cylinders, and any geometry — no meridian seam, no pole stretching.
// The 3-D parity check (floor(x)+floor(y)+floor(z)) % 2 produces a space-filling
// checkerboard that reads correctly from any camera angle.
//
// Use this variant for curved or organic geometry (spheres, cylinders, terrain).
// Use custom_checker_uv.wgsl for flat/box geometry where UV alignment is desired.
//
// Uniform layout (keys sorted alphabetically per packing convention):
//   colors (2 entries):
//     "color_a" → params_0  (r,g,b,a) — first check colour
//     "color_b" → params_1  (r,g,b,a) — second check colour
//   floats (1 entry):
//     "tiling"  → params_2.x           — cells per world unit
//                 (0.5 = 2-unit cells, 1.0 = 1-unit cells, 2.0 = 0.5-unit cells)
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tiling = max(material.params_2.x, 0.001);
    let pos    = in.world_position.xyz * tiling;

    // fract(sum * 0.5) is 0.0 for even sums and 0.5 for odd sums — works correctly
    // for both positive and negative world coordinates without integer casts.
    let sum   = floor(pos.x) + floor(pos.y) + floor(pos.z);
    let check = fract(sum * 0.5);

    if check < 0.25 {
        return material.params_0;
    } else {
        return material.params_1;
    }
}

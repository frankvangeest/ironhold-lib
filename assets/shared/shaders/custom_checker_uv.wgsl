#import bevy_pbr::forward_io::VertexOutput

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// Procedural UV-space checkerboard.
//
// Tiles the checkerboard according to the mesh's UV coordinates. Produces
// clean, texture-aligned results on cuboids, planes, and any mesh with a
// well-defined UV unwrap. On spheres or cylinders the UV seam will show as
// a misaligned row of checks; use custom_checker_world.wgsl for those shapes.
//
// Uniform layout (keys sorted alphabetically per packing convention):
//   colors (2 entries):
//     "color_a" → params_0  (r,g,b,a) — first check colour
//     "color_b" → params_1  (r,g,b,a) — second check colour
//   floats (1 entry):
//     "tiling"  → params_2.x           — UV repetitions across the mesh surface
//                 (4 = 4 checks across the full UV [0,1] range)
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tiling = max(material.params_2.x, 1.0);
    let uv     = in.uv * tiling;
    let check  = (floor(uv.x) + floor(uv.y)) % 2.0;

    if check < 0.5 {
        return material.params_0;
    } else {
        return material.params_1;
    }
}

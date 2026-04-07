#import bevy_pbr::forward_io::VertexOutput

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// Procedural checkerboard shader.
// params_0 = color_a (r,g,b,a)
// params_1 = color_b (r,g,b,a)
// params_2.x = tiling (default 8.0)
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tiling = max(material.params_2.x, 1.0);
    let uv = in.uv * tiling;
    let check = (floor(uv.x) + floor(uv.y)) % 2.0;
    if check < 0.5 {
        return material.params_0;
    } else {
        return material.params_1;
    }
}

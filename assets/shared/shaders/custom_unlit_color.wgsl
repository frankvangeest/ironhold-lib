#import bevy_pbr::forward_io::VertexOutput

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// Solid unlit color shader.
// params_0 = (r, g, b, a)  — the output color
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return material.params_0;
}

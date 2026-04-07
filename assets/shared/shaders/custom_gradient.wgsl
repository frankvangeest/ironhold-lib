#import bevy_pbr::forward_io::VertexOutput

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// UV gradient between two colors along the V axis.
// params_0 = bottom color (r,g,b,a)
// params_1 = top color    (r,g,b,a)
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return mix(material.params_0, material.params_1, in.uv.y);
}

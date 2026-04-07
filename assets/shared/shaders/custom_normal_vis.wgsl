#import bevy_pbr::forward_io::VertexOutput

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// Visualizes the world-space surface normal as an RGB color.
// (1,0,0) → red (+X), (0,1,0) → green (+Y), (0,0,1) → blue (+Z)
// Negative normal components map to black.
// No uniforms consumed — useful for debugging geometry orientation.
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    return vec4<f32>(max(n, vec3<f32>(0.0)), 1.0);
}

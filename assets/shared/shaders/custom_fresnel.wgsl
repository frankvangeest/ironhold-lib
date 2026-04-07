#import bevy_pbr::forward_io::VertexOutput

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// Fresnel rim effect.
// params_0 = rim color    (r,g,b,a)
// params_1 = base color   (r,g,b,a)
// params_2.x = rim power  (default 3.0 — higher = tighter rim)
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let rim_power = max(material.params_2.x, 0.1);
    // Approximate fresnel using the world normal's Y component as a proxy.
    // The normal's dot with view direction is approximated here from the
    // geometric normal; abs(normal.y) gives a basic edge highlight.
    let edge = 1.0 - abs(in.world_normal.y);
    let fresnel = pow(clamp(edge, 0.0, 1.0), rim_power);
    return mix(material.params_1, material.params_0, fresnel);
}

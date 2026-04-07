#import bevy_pbr::forward_io::VertexOutput

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// Horizontal world-space stripes.
// params_0 = stripe color A (r,g,b,a)
// params_1 = stripe color B (r,g,b,a)
// params_2.x = stripe frequency (world units per stripe pair, default 2.0)
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let freq = max(material.params_2.x, 0.01);
    let band = floor(in.world_position.y / freq) % 2.0;
    if band < 0.5 {
        return material.params_0;
    } else {
        return material.params_1;
    }
}

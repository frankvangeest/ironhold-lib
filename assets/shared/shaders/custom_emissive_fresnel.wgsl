#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// Emissive fresnel — transparent/dark in the centre, bright/opaque at the edges.
//
// Use with `unlit: true` and either:
//   alpha_mode: Blend  — glass-orb look: centre is translucent, rim is solid.
//   alpha_mode: Add    — energy-field look: centre is invisible, rim adds light.
//
// Uniform layout (keys sorted alphabetically per packing convention):
//   colors (2 entries):
//     "base_color"  → params_0  (r,g,b,a)  — centre colour and alpha
//     "rim_color"   → params_1  (r,g,b,a)  — edge colour and alpha
//   floats (1 entry):
//     "rim_power"   → params_2.x            — fresnel sharpness (try 2–6; higher = tighter rim)
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let base_color = material.params_0;
    let rim_color  = material.params_1;
    let rim_power  = max(material.params_2.x, 0.1);

    // True view-dependent fresnel: silhouette edges glow regardless of
    // which direction the surface is facing relative to world-up.
    let view_dir = normalize(view.world_position - in.world_position.xyz);
    let n_dot_v  = dot(normalize(in.world_normal), view_dir);
    let fresnel  = pow(1.0 - abs(n_dot_v), rim_power);

    let color = mix(base_color.rgb, rim_color.rgb, fresnel);
    let alpha = mix(base_color.a,   rim_color.a,   fresnel);
    return vec4<f32>(color, alpha);
}

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::pbr_fragment::pbr_input_from_vertex_output
#import bevy_pbr::pbr_functions::apply_pbr_lighting

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// Data-driven PBR shader — all surface properties come from RON uniforms.
//
// Uniform layout (keys sorted alphabetically per packing convention):
//   colors (1 entry):
//     "base_color"           → params_0      (r, g, b, a)
//   floats (2 entries, packed into params_1):
//     "metallic"             → params_1.x    0 = dielectric … 1 = full metal
//     "perceptual_roughness" → params_1.y    0 = mirror … 1 = fully rough
//
// Routes through Bevy's full PBR lighting pipeline so it responds to scene
// lights, shadows, ambient occlusion, and IBL environment maps.
@fragment
fn fragment(
    mesh: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {
    var pbr = pbr_input_from_vertex_output(mesh, is_front, false);
    pbr.material.base_color           = material.params_0;
    pbr.material.metallic             = material.params_1.x;
    pbr.material.perceptual_roughness = material.params_1.y;
    return apply_pbr_lighting(pbr);
}

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// Fresnel rim effect — bright at grazing angles, dark face-on.
//
// Uses the true view-dependent N·V dot product so the rim sits at the silhouette
// edges regardless of the surface orientation in the world. Works correctly on
// spheres, cylinders, and any other geometry.
//
// Uniform layout (keys sorted alphabetically per packing convention):
//   colors (2 entries):
//     "color_a"   → params_0  (r,g,b,a) — rim colour (grazing angle)
//     "color_b"   → params_1  (r,g,b,a) — face colour (normal to camera)
//   floats (1 entry):
//     "rim_power" → params_2.x           — fresnel sharpness (2=wide, 5=tight; try 2.5–4)
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let rim_power = max(material.params_2.x, 0.1);

    let view_dir = normalize(view.world_position - in.world_position.xyz);
    let n_dot_v  = abs(dot(normalize(in.world_normal), view_dir));
    let fresnel  = pow(1.0 - n_dot_v, rim_power);

    return mix(material.params_1, material.params_0, fresnel);
}

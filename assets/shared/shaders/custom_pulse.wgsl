#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// Pulsing emissive — a colour that breathes in and out over time.
//
// Use with `unlit: true` and either:
//   alpha_mode: Add    — pulses from invisible to bright; good for collectibles and effects.
//   alpha_mode: Blend  — pulses from dim to full; good for warning indicators.
//   alpha_mode: Opaque — surface colour pulses in brightness; no transparency.
//
// Uniform layout (keys sorted alphabetically per packing convention):
//   colors (1 entry):
//     "color"      → params_0  (r,g,b,a)  — peak colour and alpha
//   floats (2 entries):
//     "min_alpha"  → params_1.x            — brightness floor (0 = dark, 1 = no pulse)
//     "speed"      → params_1.y            — pulses per second (try 0.5–3.0)
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let color     = material.params_0;
    let min_alpha = clamp(material.params_1.x, 0.0, 1.0);
    let speed     = max(material.params_1.y, 0.01);

    // Smooth sine pulse: oscillates between min_alpha and 1.0.
    let pulse      = 0.5 + 0.5 * sin(globals.time * speed * 6.283185);
    let brightness = mix(min_alpha, 1.0, pulse);

    return vec4<f32>(color.rgb * brightness, color.a * brightness);
}

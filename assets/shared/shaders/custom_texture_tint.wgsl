#import bevy_pbr::forward_io::VertexOutput

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var texture_0: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var sampler_0: sampler;

// Texture with a colour tint multiplied on top.
// The first shader in the shared library that samples a texture.
//
// A tint of (1, 1, 1, 1) is a no-op — the texture is output as-is.
// Reduce the tint's RGB to shift hue; reduce alpha to make the surface
// translucent (combine with alpha_mode: Blend for transparency).
//
// Uniform layout:
//   colors (1 entry):
//     "tint"       → params_0  (r,g,b,a)  — multiplied with the texture sample
//
// RON example:
//   "my_tinted_mat": (
//     kind: Custom((
//       shader: Some("shared/shaders/custom_texture_tint.wgsl"),
//       textures: { "texture_0": "shared/textures/wood_crate_albedo.png" },
//       colors: { "tint": (r:1.0, g:0.4, b:0.2, a:1.0) },
//     )),
//   ),
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tint    = material.params_0;
    let texel   = textureSample(texture_0, sampler_0, in.uv);
    return texel * tint;
}

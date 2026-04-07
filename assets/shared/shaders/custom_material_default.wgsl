// custom_material_default.wgsl
//
// Fallback fragment shader rendered when a CustomMaterial is created without
// a valid `shader` path in the RON. Renders solid magenta so it is immediately
// obvious in the viewport that the material is not configured.
//
// This shader is embedded at compile time and never loaded from disk at runtime.

#import bevy_pbr::forward_io::VertexOutput

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Magenta = "missing custom shader" indicator.
    return vec4<f32>(1.0, 0.0, 1.0, 1.0);
}

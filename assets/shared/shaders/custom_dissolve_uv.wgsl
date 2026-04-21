#import bevy_pbr::forward_io::VertexOutput

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// Procedural dissolve — UV-space variant.
//
// Uses the mesh UV coordinates for the noise input so the dissolve pattern
// follows the surface's texture layout. Clean on cuboids and flat planes.
// On spheres or cylinders the UV seam will show as a sharp discontinuity;
// use custom_dissolve_world.wgsl for those shapes.
//
// Use with `alpha_mode: Mask(0.5)`. The shader outputs alpha=1 for visible
// fragments and alpha=0 for cut fragments; Bevy discards anything below 0.5.
// Fragments near the cut edge receive `edge_color` for a burning/glowing rim.
//
// Uniform layout (keys sorted alphabetically per packing convention):
//   colors (2 entries):
//     "base_color"   → params_0  (r,g,b,a)  — visible surface colour
//     "edge_color"   → params_1  (r,g,b,a)  — glow colour at the dissolve edge
//   floats (3 entries):
//     "edge_width"   → params_2.x            — glowing rim width (try 0.05–0.2)
//     "threshold"    → params_2.y            — cut position: 0 = fully visible, 1 = fully dissolved
//     "tiling"       → params_2.z            — UV noise scale (4–12; higher = finer grain)

// ── Noise helpers ─────────────────────────────────────────────────────────────

fn hash2(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(0.1031, 0.1030));
    q += dot(q, q.yx + 33.33);
    return fract((q.x + q.y) * q.x);
}

fn value_noise(uv: vec2<f32>) -> f32 {
    let i = floor(uv);
    let f = fract(uv);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash2(i + vec2<f32>(0.0, 0.0)), hash2(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash2(i + vec2<f32>(0.0, 1.0)), hash2(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
}

fn fbm(uv: vec2<f32>) -> f32 {
    var value     = 0.0;
    var amplitude = 0.5;
    var freq      = 1.0;
    for (var i = 0; i < 4; i++) {
        value     += amplitude * value_noise(uv * freq);
        amplitude *= 0.5;
        freq      *= 2.1;
    }
    return value;
}

// ── Fragment ──────────────────────────────────────────────────────────────────

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let base_color = material.params_0;
    let edge_color = material.params_1;
    let edge_width = max(material.params_2.x, 0.001);
    let threshold  = clamp(material.params_2.y, 0.0, 1.0);
    let tiling     = max(material.params_2.z, 0.1);

    let noise = fbm(in.uv * tiling);

    let above   = noise - threshold;
    let visible = step(0.0, above);

    let edge_t = smoothstep(0.0, edge_width, above) * visible;
    let color  = mix(edge_color.rgb, base_color.rgb, edge_t);

    return vec4<f32>(color, visible);
}

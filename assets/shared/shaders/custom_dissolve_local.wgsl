#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_functions::get_world_from_local

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// Procedural dissolve — local (object-space) variant. Seamless on any geometry.
//
// Identical to custom_dissolve_world.wgsl but the noise is sampled in the
// object's local coordinate frame instead of world space. Each object shows
// the same centred dissolve pattern regardless of its world position — moving
// the object does not shift the cut boundary.
//
// Use with `alpha_mode: Mask(0.5)`.
//
// Uniform layout (keys sorted alphabetically per packing convention):
//   colors (2 entries):
//     "base_color"   → params_0  (r,g,b,a)  — visible surface colour
//     "edge_color"   → params_1  (r,g,b,a)  — glow colour at the dissolve edge
//   floats (3 entries):
//     "edge_width"   → params_2.x            — glowing rim width (try 0.05–0.2)
//     "threshold"    → params_2.y            — cut position: 0 = fully visible, 1 = fully dissolved
//     "tiling"       → params_2.z            — noise scale in local units (0.5–2.0)

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

fn fbm(p: vec2<f32>) -> f32 {
    var value     = 0.0;
    var amplitude = 0.5;
    var freq      = 1.0;
    for (var i = 0; i < 4; i++) {
        value     += amplitude * value_noise(p * freq);
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

    // Recover local XZ position via TRS-matrix inverse (same technique as
    // custom_checker_local.wgsl). Noise is sampled on the local horizontal
    // plane so the dissolve pattern always reads cleanly from above.
    let m    = get_world_from_local(in.instance_index);
    let col0 = m[0].xyz;
    let col2 = m[2].xyz;
    let rel  = in.world_position.xyz - m[3].xyz;
    let local_xz = vec2<f32>(
        dot(rel, col0) / dot(col0, col0),
        dot(rel, col2) / dot(col2, col2),
    );

    let noise = fbm(local_xz * tiling);

    let above   = noise - threshold;
    let visible = step(0.0, above);
    let edge_t  = smoothstep(0.0, edge_width, above) * visible;
    let color   = mix(edge_color.rgb, base_color.rgb, edge_t);

    return vec4<f32>(color, visible);
}

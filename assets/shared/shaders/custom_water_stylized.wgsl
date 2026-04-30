// custom_water_stylized.wgsl
// Animated stylised water — fragment-only, no vertex displacement.
//
// Ripples are driven entirely from world-space XZ position so there are no
// UV seams regardless of mesh shape.  Three overlapping sine waves at different
// frequencies and travel directions create a natural-looking ripple field.
// A Fresnel term makes the surface brighter and more opaque at grazing angles,
// giving a shoreline-foam illusion.  Wave crests add a brief sparkle boost.
//
// Recommended settings:
//   unlit: true
//   alpha_mode: Blend
//   double_sided: false   (water seen from above only)
//
// ── Uniform layout ────────────────────────────────────────────────────────────
// (colors first, floats second — both sorted alphabetically per packing rules)
//
//   params_0 = deep_color    (r,g,b,a) — centre colour when looking straight down
//   params_1 = shallow_color (r,g,b,a) — rim colour at grazing angle (foam/sparkle)
//   params_2.x = flow_speed    — wave animation rate (try 0.3–1.5)
//   params_2.y = fresnel_power — Fresnel exponent; higher = tighter rim (try 1.5–4.0)
//   params_2.z = tiling        — wave spatial scale; higher = more ripples per unit (try 0.3–1.0)
//   params_2.w = wave_strength — normal perturbation amplitude (try 0.1–0.35)
//
// ── Textures ──────────────────────────────────────────────────────────────────
// None required.  All four slots receive a 1×1 white fallback from the engine.
// The texture bindings below are declared to satisfy the CustomMaterial binding
// contract and are never sampled.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals
#import bevy_pbr::mesh_view_bindings::view

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var texture_0: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var sampler_0: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var texture_1: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var sampler_1: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var texture_2: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var sampler_2: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var texture_3: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var sampler_3: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let deep_color    = material.params_0;
    let shallow_color = material.params_1;
    let flow_speed    = material.params_2.x;
    let fresnel_power = max(material.params_2.y, 0.1);
    let tiling        = material.params_2.z;
    let wave_strength = material.params_2.w;

    let t    = globals.time * flow_speed;
    let wpos = in.world_position.xz * tiling;

    // Three sine waves with different travel directions and frequencies.
    // The mix of co-prime-ish direction vectors prevents a regular grid pattern.
    let w1 = sin(dot(wpos, vec2<f32>( 1.00,  0.80)) + t * 1.00);
    let w2 = sin(dot(wpos, vec2<f32>(-0.70,  1.00)) + t * 1.30);
    let w3 = sin(dot(wpos, vec2<f32>( 2.50, -1.80)) + t * 0.70) * 0.50;
    let wave = (w1 + w2 + w3) * (1.0 / 3.0);  // range -1..1

    // Gradient of the wave field in wpos space — used to perturb the surface
    // normal so Fresnel varies across the surface, simulating ripple highlights.
    let dw_dx = cos(dot(wpos, vec2<f32>( 1.00,  0.80)) + t * 1.00) *  1.00
              + cos(dot(wpos, vec2<f32>(-0.70,  1.00)) + t * 1.30) * (-0.70)
              + cos(dot(wpos, vec2<f32>( 2.50, -1.80)) + t * 0.70) *  2.50 * 0.50;
    let dw_dz = cos(dot(wpos, vec2<f32>( 1.00,  0.80)) + t * 1.00) *  0.80
              + cos(dot(wpos, vec2<f32>(-0.70,  1.00)) + t * 1.30) *  1.00
              + cos(dot(wpos, vec2<f32>( 2.50, -1.80)) + t * 0.70) * (-1.80) * 0.50;
    let perturbed_n = normalize(vec3<f32>(-dw_dx * wave_strength, 1.0, -dw_dz * wave_strength));

    // Fresnel: surface appears more opaque and brighter at grazing view angles.
    let view_dir = normalize(view.world_position - in.world_position.xyz);
    let n_dot_v  = max(dot(perturbed_n, view_dir), 0.0);
    let fresnel  = pow(1.0 - n_dot_v, fresnel_power);

    var rgb   = mix(deep_color.rgb, shallow_color.rgb, fresnel);
    var alpha = mix(deep_color.a,   shallow_color.a,   fresnel);

    // Foam sparkle at wave crests: briefly brightens the highlights.
    let crest = smoothstep(0.50, 0.85, wave);
    rgb   = rgb   + shallow_color.rgb * crest * 0.30;
    alpha = min(alpha + crest * 0.20, 1.0);

    return vec4<f32>(rgb, alpha);
}

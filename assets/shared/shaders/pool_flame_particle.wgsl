// pool_flame_particle.wgsl
//
// Fragment shader for pooled UV-animated flame billboard particles.
// Per-particle elapsed time is encoded in mesh UV1 (ATTRIBUTE_UV_1), so all
// particles in the same render group share one material handle.
//
// Binding contract (matches PoolFlameMaterial in particle_renderer.rs):
//   @binding(0)  PoolFlameUniforms { params: vec4 }  — scroll_speed, distort, 0, 0
//   @binding(1)  texture_2d  (flame sprite)
//   @binding(2)  sampler
//
// Per-vertex data from the CPU pool:
//   in.uv       — sprite UV [0,1]^2
//   in.uv_b.x   — particle elapsed time in seconds (from ATTRIBUTE_UV_1)
//   in.color    — RGBA particle colour from the colour gradient

#import bevy_pbr::forward_io::VertexOutput

struct PoolFlameUniforms {
    params: vec4<f32>,  // x=scroll_speed  y=distort_strength  zw=unused
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material:     PoolFlameUniforms;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var          flame_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var          flame_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t       = in.uv_b.x;        // per-particle elapsed seconds
    let scroll  = material.params.x;
    let distort = material.params.y;

    var uv = in.uv;

    // Tip-weighted UV distortion — same formula as custom_flame_particle.wgsl.
    // tip_w is 1 at the tip (uv.y≈0) and 0 at the base, so distortion concentrates
    // where a real flame actually wavers.
    if distort > 0.001 {
        let tip_w  = 1.0 - uv.y;
        let tip_w2 = tip_w * tip_w;

        uv.x += sin(t * 2.1 + uv.y * 2.4) * 0.10 * tip_w2 * distort;
        uv.x += sin(t * 5.7 + uv.y * 6.3) * 0.05 * tip_w2 * distort;
        uv.y += sin(t * 3.8 + uv.x * 4.9) * 0.04 * tip_w  * distort;
    }

    uv.y -= t * scroll;
    uv = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));

    let tex = textureSample(flame_texture, flame_sampler, uv);
    return vec4<f32>(tex.rgb * in.color.rgb, tex.a * in.color.a);
}

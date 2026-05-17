// custom_flame_particle.wgsl
//
// Fragment shader for animated flame/fire billboard particles.
//
// Binding contract (matches FlameParticleMaterial in flame_material.rs):
//   @binding(0)  FlameUniforms { color: vec4, params: vec4 }
//   @binding(1)  sprite texture
//   @binding(2)  sprite sampler
//
// Uniforms:
//   color  — linear-space RGBA tint; updated every frame from the particle
//             colour gradient (start → mid → end).
//   params — (scroll_speed, distort_strength, _unused_, elapsed_time)
//             scroll_speed    : texture UV.y advance per second (upward scroll)
//             distort_strength: amplitude of tip-weighted UV distortion [0..1]
//             elapsed_time    : particle age in seconds; updated every frame
//
// The shader applies tip-weighted UV distortion using overlapping sine waves so
// the flame tip dances while the base stays grounded, then blends in optional
// upward scroll. UVs are clamped (not wrapped) so the sprite never tiles.
//
// For additive blending the alpha channel controls how much colour is added to
// the scene — no pre-multiplication needed because AlphaMode::Add handles that.

#import bevy_pbr::forward_io::VertexOutput

struct FlameUniforms {
    color:  vec4<f32>,  // linear RGBA tint — updated every frame
    params: vec4<f32>,  // x=scroll_speed  y=distort_strength  z=_unused_  w=elapsed_time
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: FlameUniforms;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var sprite:         texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var sprite_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t       = material.params.w;   // elapsed time (seconds)
    let scroll  = material.params.x;   // UV scroll speed (texture heights / second)
    let distort = material.params.y;   // distortion strength [0..1]

    var uv = in.uv;

    // ── Tip-weighted distortion ───────────────────────────────────────────────
    // Kenney flame sprites have the tip at uv.y≈0 and the base at uv.y≈1.
    // tip_w is 1.0 at the tip and 0.0 at the base so distortion is concentrated
    // where a real flame actually wavers, leaving the base visually grounded.
    if distort > 0.001 {
        let tip_w  = 1.0 - uv.y;          // linear tip weight
        let tip_w2 = tip_w * tip_w;        // squared: focuses effect at very tip

        // Primary sway — slow left-right lean of the whole flame
        uv.x += sin(t * 2.1 + uv.y * 2.4) * 0.10 * tip_w2 * distort;
        // Secondary flutter — faster, smaller, irregular flicker
        uv.x += sin(t * 5.7 + uv.y * 6.3) * 0.05 * tip_w2 * distort;
        // Vertical ripple — slight height-wise stretch at the tip
        uv.y += sin(t * 3.8 + uv.x * 4.9) * 0.04 * tip_w * distort;
    }

    // ── Upward scroll ─────────────────────────────────────────────────────────
    // Decreasing uv.y over time moves the sampled region toward the tip,
    // making the texture appear to rise.  Clamped (not wrapped) so the sprite
    // does not tile — the tip row simply persists once reached.
    uv.y -= t * scroll;

    // Clamp to [0, 1] — no tiling seam, sprite stays within its boundary.
    uv = clamp(uv, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));

    let tex = textureSample(sprite, sprite_sampler, uv);

    // Kenney sprites are grayscale RGBA (R=G=B=A).  The tint colour drives the
    // final hue; the texture provides the shape and softness.
    return vec4<f32>(tex.rgb * material.color.rgb, tex.a * material.color.a);
}

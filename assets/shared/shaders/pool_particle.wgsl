// pool_particle.wgsl
//
// Fragment shader for pooled billboard particles (additive and blend modes).
// Per-particle colour comes from vertex colours (ATTRIBUTE_COLOR on the mesh),
// animated by the CPU simulation each frame.  The optional texture provides
// the particle shape/softness and is multiplied into the vertex colour.
//
// Binding contract (matches PoolParticleMaterial in particle_renderer.rs):
//   @binding(0)  texture_2d (particle sprite, or Bevy's default 1×1 white)
//   @binding(1)  sampler

#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var pool_texture:  texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var pool_sampler:  sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex   = textureSample(pool_texture, pool_sampler, in.uv);
    let color = in.color;
    return vec4<f32>(tex.rgb * color.rgb, tex.a * color.a);
}

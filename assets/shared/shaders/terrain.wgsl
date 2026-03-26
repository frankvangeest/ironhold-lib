#import bevy_pbr::forward_io::VertexOutput

struct TerrainMaterial {
    uv_scale: vec4<f32>,  // Only .x is used; padded for WebGPU 16-byte alignment
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: TerrainMaterial;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var splatmap: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var splatmap_sampler: sampler;

@group(#{MATERIAL_BIND_GROUP}) @binding(3) var texture_r: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var sampler_r: sampler;

@group(#{MATERIAL_BIND_GROUP}) @binding(5) var texture_g: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var sampler_g: sampler;

@group(#{MATERIAL_BIND_GROUP}) @binding(7) var texture_b: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var sampler_b: sampler;

@group(#{MATERIAL_BIND_GROUP}) @binding(9) var texture_a: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(10) var sampler_a: sampler;

@fragment
fn fragment(
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {
    let splat = textureSample(splatmap, splatmap_sampler, mesh.uv);
    let uv_tiled = mesh.uv * material.uv_scale.x;
    
    let col_r = textureSample(texture_r, sampler_r, uv_tiled);
    let col_g = textureSample(texture_g, sampler_g, uv_tiled);
    let col_b = textureSample(texture_b, sampler_b, uv_tiled);
    let col_a = textureSample(texture_a, sampler_a, uv_tiled);

    // Weighted sum of textures based on splatmap channels
    var color = col_r * splat.r + col_g * splat.g + col_b * splat.b + col_a * splat.a;
    
    // Normalize if channels don't sum to 1.0 (conservative fallback)
    let total_weight = splat.r + splat.g + splat.b + splat.a;
    if (total_weight < 0.01) {
        color = vec4<f32>(0.2, 0.2, 0.2, 1.0);
    } else if (total_weight > 1.01) {
        color /= total_weight;
    }

    return color;
}

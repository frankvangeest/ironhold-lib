#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::pbr_fragment::pbr_input_from_vertex_output
#import bevy_pbr::pbr_functions::apply_pbr_lighting

struct TerrainMaterial {
    uv_scale: vec4<f32>,  // .x = UV tiling factor; .yzw padded for 16-byte alignment
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
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {
    // Sample splatmap for per-layer blend weights.
    // Only the RGB channels are used: R→layer0, G→layer1, B→layer2.
    // The alpha channel is ignored because the splatmap is an RGB PNG (Bevy
    // pads A=1.0 on load, which would otherwise corrupt every blend weight).
    let splat_raw = textureSample(splatmap, splatmap_sampler, mesh.uv).rgb;
    let uv_tiled = mesh.uv * material.uv_scale.x;

    // Sample the first three texture layers (texture_a is unused with RGB splatmaps)
    let col_r = textureSample(texture_r, sampler_r, uv_tiled);
    let col_g = textureSample(texture_g, sampler_g, uv_tiled);
    let col_b = textureSample(texture_b, sampler_b, uv_tiled);

    // Normalise weights so they always sum to 1.0; fallback grey if splatmap is black
    let total_weight = splat_raw.r + splat_raw.g + splat_raw.b;
    var base_color: vec4<f32>;
    if total_weight < 0.01 {
        base_color = vec4<f32>(0.2, 0.2, 0.2, 1.0);
    } else {
        let w = splat_raw / total_weight;
        base_color = col_r * w.r + col_g * w.g + col_b * w.b;
    }
    base_color.a = 1.0;  // terrain is always fully opaque

    // Route through Bevy's PBR lighting pipeline so the terrain responds to
    // scene lights, shadows, ambient, and the IBL environment map.
    var pbr_input = pbr_input_from_vertex_output(mesh, is_front, false);
    pbr_input.material.base_color = base_color;
    pbr_input.material.perceptual_roughness = 0.85;
    pbr_input.material.metallic = 0.0;
    return apply_pbr_lighting(pbr_input);
}

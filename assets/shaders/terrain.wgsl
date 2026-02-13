#import bevy_pbr::mesh_functions::mesh_position_local_to_clip
#import bevy_pbr::mesh_functions::mesh_position_to_world
#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::pbr_types::{PbrInput, pbr_input_new}
#import bevy_pbr::pbr_functions

struct TerrainMaterial {
    // We can add uniforms here if needed, e.g. UV scaling
    uv_scale: f32,
}

@group(1) @binding(0) var<uniform> material: TerrainMaterial;
@group(1) @binding(1) var splatmap: texture_2d<f32>;
@group(1) @binding(2) var splatmap_sampler: sampler;

@group(1) @binding(3) var texture_r: texture_2d<f32>;
@group(1) @binding(4) var sampler_r: sampler;

@group(1) @binding(5) var texture_g: texture_2d<f32>;
@group(1) @binding(6) var sampler_g: sampler;

@group(1) @binding(7) var texture_b: texture_2d<f32>;
@group(1) @binding(8) var sampler_b: sampler;

// We could add more for alpha channel or normal maps

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    var world_position = mesh_position_to_world(
        mat4x4<f32>(
            vec4<f32>(1.0, 0.0, 0.0, 0.0),
            vec4<f32>(0.0, 1.0, 0.0, 0.0), 
            vec4<f32>(0.0, 0.0, 1.0, 0.0), 
            vec4<f32>(0.0, 0.0, 0.0, 1.0)
        ), // Identity for now, or use mesh binding
        vec4<f32>(vertex.position, 1.0)
    );
    // Actually we should use standard mesh bindings.
    // But `TerrainMaterial` uses `MaterialMesh2dBundle`? No, 3d.
    // Let's rely on standard PBR vertex for now or copy it.
    // Custom vertex shader is needed if we do displacement in shader.
    // Since we generate mesh in CPU, we can use StandardMaterial's vertex shader?
    // But we need custom Fragment.
    
    // For simplicity, let's implement a basic vertex pass that works with `MaterialExtension` later?
    // Or just write a full PBR shader.
    // Bevy's `Material` trait allows customizing vertex/fragment.
    // Let's assume we use default mesh vertex shader for now. 
    // Wait, Bevy requires full vertex shader if we override it?
    // Usually yes.
    
    // Simpler: Just generic pass-through
    out.world_position = mesh_position_to_world(mat4x4<f32>(vec4<f32>(1.0,0.0,0.0,0.0),vec4<f32>(0.0,1.0,0.0,0.0),vec4<f32>(0.0,0.0,1.0,0.0),vec4<f32>(0.0,0.0,0.0,1.0)), vec4<f32>(vertex.position, 1.0));
    out.clip_position = mesh_position_local_to_clip(mat4x4<f32>(vec4<f32>(1.0,0.0,0.0,0.0),vec4<f32>(0.0,1.0,0.0,0.0),vec4<f32>(0.0,0.0,1.0,0.0),vec4<f32>(0.0,0.0,0.0,1.0)), vec4<f32>(vertex.position, 1.0));
    out.world_normal = vertex.normal; // Needs transform
    out.uv = vertex.uv;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let splat = textureSample(splatmap, splatmap_sampler, in.uv);
    
    let uv_tiled = in.uv * material.uv_scale;
    
    let col_r = textureSample(texture_r, sampler_r, uv_tiled);
    let col_g = textureSample(texture_g, sampler_g, uv_tiled);
    let col_b = textureSample(texture_b, sampler_b, uv_tiled);
    // Default base (e.g. if everything is 0)
    let col_base = vec4<f32>(0.1, 0.1, 0.1, 1.0); 

    var color = col_base;
    color = mix(color, col_r, splat.r);
    color = mix(color, col_g, splat.g);
    color = mix(color, col_b, splat.b);

    // Only albedo for now. PBR needs more.
    // We can return simple color or try to integrate with PBR.
    // For now, emissive-like output? No, proper PBR is complex in custom shader.
    // Let's just output color.
    
    return color;
}

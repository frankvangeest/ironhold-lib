// foliage_prepass.wgsl
//
// Shadow / depth prepass shader for FoliageMaterial.
//
// Performs the same billboard vertex expansion as the main foliage.wgsl so the
// shadow map receives correctly-positioned leaf card geometry.  The fragment
// stage then alpha-clips the leaf texture, producing leaf-shaped shadows
// instead of the default rectangular quad silhouette.
//
// In the shadow pass Bevy binds the light's view matrix as `view`, so
// `view.world_from_view` gives the light's right/up vectors — the billboard
// expansion formula is identical to the camera-facing case.

#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::mesh_functions::get_world_from_local

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var leaf_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var leaf_sampler: sampler;

struct PrepassVertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position:    vec3<f32>,   // corner offset from leaf anchor
    @location(1) normal:      vec3<f32>,   // sphere normal — declared to match vertex buffer layout
    @location(2) uv:          vec2<f32>,
    @location(10) leaf_center: vec3<f32>,  // leaf anchor (ATTRIBUTE_LEAF_CENTER)
}

struct PrepassOutput {
    @builtin(position) position: vec4<f32>,
    @location(0)       uv:       vec2<f32>,
}

@vertex
fn prepass_vertex(in: PrepassVertex) -> PrepassOutput {
    var out: PrepassOutput;

    let model        = get_world_from_local(in.instance_index);
    let anchor_world = (model * vec4<f32>(in.leaf_center, 1.0)).xyz;

    // In the shadow pass `view` is the light's view — same formula works.
    let cam_right = view.world_from_view[0].xyz;
    let cam_up    = view.world_from_view[1].xyz;

    let world_pos = anchor_world
        + in.position.x * cam_right
        + in.position.y * cam_up;

    out.position = view.clip_from_world * vec4<f32>(world_pos, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn prepass_fragment(in: PrepassOutput) {
    // Discard transparent regions — depth is written implicitly from position.
    let alpha = textureSample(leaf_texture, leaf_sampler, in.uv).a;
    if alpha < 0.5 { discard; }
}

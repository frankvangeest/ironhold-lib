// foliage.wgsl
//
// Vertex + fragment shader for the stylized foliage system.
//
// Vertex stage:
//   - Each leaf card is stored as 4 vertices with corner offsets in
//     ATTRIBUTE_POSITION (local-space offsets from the leaf anchor) and
//     the anchor position in ATTRIBUTE_LEAF_CENTER (location 10).
//   - The billboard is built by transforming the anchor to world space
//     and then expanding the corner offset along the camera right/up vectors,
//     producing a quad that always faces the camera regardless of entity rotation.
//   - ATTRIBUTE_NORMAL carries the sphere normal baked at mesh-build time;
//     it is transformed by the model matrix for correct world-space lighting.
//
// Fragment stage:
//   - Alpha-clip at 0.5 (hard discard for painterly silhouettes, no blending).
//   - 2-, 3-, or 4-band toon cel shading using the sphere-mapped world normal.
//   - AO darkening on the shadow hemisphere.

#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::mesh_functions::get_world_from_local
#import bevy_render::view::View

// ─── Material uniforms ────────────────────────────────────────────────────────

struct FoliageMaterialParams {
    color_highlight: vec4<f32>,   // rgb + padding
    color_midtone:   vec4<f32>,   // rgb + padding
    color_shadow:    vec4<f32>,   // rgb + padding
    sun_direction:   vec4<f32>,   // xyz + padding; updated each frame
    // x = ao_intensity, y = toon_bands (2.0, 3.0, or 4.0), zw = unused
    config:          vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var leaf_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var leaf_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> material: FoliageMaterialParams;

// ─── Vertex stage ─────────────────────────────────────────────────────────────

struct FoliageVertexInput {
    @builtin(instance_index) instance_index: u32,
    // Corner offset from the leaf anchor point, in the prefab's local space.
    @location(0) position:    vec3<f32>,
    // Sphere normal: points from cluster centre through the leaf anchor.
    @location(1) normal:      vec3<f32>,
    @location(2) uv:          vec2<f32>,
    // Leaf anchor position in the prefab's local space (ATTRIBUTE_LEAF_CENTER).
    @location(10) leaf_center: vec3<f32>,
}

struct FoliageVertexOutput {
    @builtin(position) position:    vec4<f32>,
    @location(0)       world_normal: vec3<f32>,
    @location(1)       uv:           vec2<f32>,
}

@vertex
fn vertex(in: FoliageVertexInput) -> FoliageVertexOutput {
    var out: FoliageVertexOutput;

    let model = get_world_from_local(in.instance_index);

    // Transform the leaf anchor to world space.
    let anchor_world = (model * vec4<f32>(in.leaf_center, 1.0)).xyz;

    // Camera right and up vectors in world space — extracted from the inverse
    // view matrix (world_from_view): column 0 = right, column 1 = up.
    let cam_right = view.world_from_view[0].xyz;
    let cam_up    = view.world_from_view[1].xyz;

    // Expand the corner offset in view space to produce the billboard position.
    let world_pos = anchor_world
        + in.position.x * cam_right
        + in.position.y * cam_up;

    out.position     = view.clip_from_world * vec4<f32>(world_pos, 1.0);
    out.world_normal = normalize((model * vec4<f32>(in.normal, 0.0)).xyz);
    out.uv           = in.uv;

    return out;
}

// ─── Toon shading helpers ─────────────────────────────────────────────────────

fn toon_tone(NdotL: f32, bands: f32) -> f32 {
    let b = i32(bands + 0.5);
    if b <= 2 {
        return select(0.0, 1.0, NdotL > 0.0);
    } else if b == 3 {
        if NdotL > 0.5 { return 1.0; }
        else if NdotL > 0.0 { return 0.5; }
        else { return 0.0; }
    } else {
        if NdotL > 0.66 { return 1.0; }
        else if NdotL > 0.33 { return 0.66; }
        else if NdotL > 0.0 { return 0.33; }
        else { return 0.0; }
    }
}

// ─── Fragment stage ───────────────────────────────────────────────────────────

@fragment
fn fragment(in: FoliageVertexOutput) -> @location(0) vec4<f32> {
    // Alpha-clip for painterly silhouette.
    let alpha = textureSample(leaf_texture, leaf_sampler, in.uv).a;
    if alpha < 0.5 { discard; }

    let sun_dir   = normalize(material.sun_direction.xyz);
    let NdotL     = dot(normalize(in.world_normal), sun_dir);
    let tone      = toon_tone(NdotL, material.config.y);

    // AO: darken based on the shadow hemisphere.
    let ao_intensity = material.config.x;
    let ao = 1.0 - ao_intensity * max(0.0, -NdotL);

    // Interpolate between the three tonal colours based on tone value.
    let col_lo = mix(material.color_shadow.rgb, material.color_midtone.rgb,
                     clamp(tone * 2.0, 0.0, 1.0));
    let col_hi = mix(material.color_midtone.rgb, material.color_highlight.rgb,
                     clamp(tone * 2.0 - 1.0, 0.0, 1.0));
    let base   = mix(col_lo, col_hi, step(0.5, tone));

    return vec4<f32>(base * ao, 1.0);
}

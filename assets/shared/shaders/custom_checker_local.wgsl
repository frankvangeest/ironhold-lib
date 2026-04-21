#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_functions::get_world_from_local

struct CustomMaterialUniforms {
    params_0: vec4<f32>,
    params_1: vec4<f32>,
    params_2: vec4<f32>,
    params_3: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;

// Procedural 3-D local (object-space) checkerboard.
//
// Identical to world-space checker but the pattern is anchored to the object's
// own coordinate frame instead of the world grid. Two spheres placed far apart
// will each show the same centered pattern — the pattern does not shift with
// the object's world translation, rotation, or scale.
//
// Local position is recovered from world position by analytically inverting the
// TRS model matrix. For a matrix without shear the inverse is exact:
//   local_i = dot(world_pos - translation, col_i) / |col_i|²
//
// Use this variant when you want each object instance to carry its own
// self-contained pattern (e.g. independent prop colouring, instanced objects).
// Use custom_checker_world.wgsl when objects should share a continuous world grid.
//
// Uniform layout (keys sorted alphabetically per packing convention):
//   colors (2 entries):
//     "color_a" → params_0  (r,g,b,a) — first check colour
//     "color_b" → params_1  (r,g,b,a) — second check colour
//   floats (1 entry):
//     "tiling"  → params_2.x           — cells per local unit
//                 (0.5 = 2-unit cells, 1.0 = 1-unit cells, 2.0 = 0.5-unit cells)
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tiling = max(material.params_2.x, 0.001);

    // Recover local position using the model matrix columns.
    // m[i] is the i-th column of the world_from_local (model) matrix.
    // Projecting world_pos - translation onto each column axis and dividing by
    // the squared column length gives the coordinate in local (object) space.
    let m    = get_world_from_local(in.instance_index);
    let col0 = m[0].xyz;
    let col1 = m[1].xyz;
    let col2 = m[2].xyz;
    let rel  = in.world_position.xyz - m[3].xyz;
    let pos  = vec3<f32>(
        dot(rel, col0) / dot(col0, col0),
        dot(rel, col1) / dot(col1, col1),
        dot(rel, col2) / dot(col2, col2),
    ) * tiling;

    let sum   = floor(pos.x) + floor(pos.y) + floor(pos.z);
    let check = fract(sum * 0.5);

    if check < 0.25 {
        return material.params_0;
    } else {
        return material.params_1;
    }
}

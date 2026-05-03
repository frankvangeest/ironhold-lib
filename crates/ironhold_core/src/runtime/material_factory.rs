use bevy::prelude::*;
use bevy::pbr::StandardMaterial;
use std::collections::HashMap;

use crate::schema::material::{AlphaModeDef, ColorDef, MaterialDef, MaterialKind};
use crate::capabilities::terrain_material::TerrainMaterial;
use crate::capabilities::custom_material::{
    CustomMaterial, CustomMaterialUniforms, CUSTOM_MATERIAL_FALLBACK_HANDLE,
};

// ---------------------------------------------------------------------------
// Built material handle — erased enum so we can store Standard, Terrain, and
// Custom handles in the same map.
// ---------------------------------------------------------------------------

pub enum BuiltMaterialHandle {
    Standard(Handle<StandardMaterial>),
    Terrain(Handle<TerrainMaterial>),
    Custom(Handle<CustomMaterial>),
}

// ---------------------------------------------------------------------------
// Resource: the project-wide material registry, built once per scene load.
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
pub struct BuiltMaterials(pub HashMap<String, BuiltMaterialHandle>);

// ---------------------------------------------------------------------------
// Component: marks an entity whose material should be replaced once its
// GLTF scene children have appeared.
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct PendingMaterialOverride(pub String); // key into BuiltMaterials

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

pub struct MaterialFactory;

impl MaterialFactory {
    /// Build a Bevy material from a catalog `MaterialDef`, returning an
    /// erased handle that can be stored in `BuiltMaterials`.
    pub fn build(
        asset_server: &AssetServer,
        standard_materials: &mut Assets<StandardMaterial>,
        terrain_materials: &mut Assets<TerrainMaterial>,
        custom_materials: &mut Assets<CustomMaterial>,
        name: &str,
        def: &MaterialDef,
    ) -> BuiltMaterialHandle {
        match &def.kind {
            MaterialKind::Standard(std_def) => {
                let handle = Self::build_standard(asset_server, standard_materials, def, std_def);
                BuiltMaterialHandle::Standard(handle)
            }
            MaterialKind::Terrain(terrain_def) => {
                if !matches!(def.alpha_mode, AlphaModeDef::Opaque) || def.double_sided || def.unlit {
                    warn!(
                        "Material '{}': alpha_mode, double_sided, and unlit are not supported for \
                         Terrain materials and will be ignored.",
                        name
                    );
                }
                let fallback = |i: usize| -> String {
                    let defaults = [
                        "shared/terrain/grass.png",
                        "shared/terrain/rock.png",
                        "shared/terrain/dirt.png",
                        "shared/terrain/snow.png",
                    ];
                    terrain_def.layers.get(i)
                        .cloned()
                        .unwrap_or_else(|| defaults[i.min(3)].to_string())
                };

                let mat = TerrainMaterial {
                    uv_scale: Vec4::new(terrain_def.uv_scale, 0.0, 0.0, 0.0),
                    splatmap:  asset_server.load(terrain_def.splatmap.clone()),
                    texture_r: asset_server.load(fallback(0)),
                    texture_g: asset_server.load(fallback(1)),
                    texture_b: asset_server.load(fallback(2)),
                    texture_a: asset_server.load(fallback(3)),
                };
                BuiltMaterialHandle::Terrain(terrain_materials.add(mat))
            }
            MaterialKind::Custom(custom_def) => {
                // Resolve the fragment shader handle.
                // If no path is specified we keep the built-in magenta fallback.
                let shader = match &custom_def.shader {
                    Some(path) if !path.is_empty() => {
                        info!("Material '{}': loading custom shader from '{}'", name, path);
                        asset_server.load::<Shader>(path.clone())
                    }
                    _ => {
                        warn!(
                            "Material '{}': no shader path specified — using magenta fallback",
                            name
                        );
                        CUSTOM_MATERIAL_FALLBACK_HANDLE
                    }
                };

                // Pack uniforms.
                // Convention (both maps sorted alphabetically by key):
                //   • Each color  → one Vec4 (r,g,b,a), filling params_0 first
                //   • Each float  → packed 4-per-Vec4 into remaining param slots
                let uniforms = pack_custom_uniforms(custom_def);

                // Resolve up to 4 named texture slots.
                let texture_0 = custom_def.textures.get("texture_0")
                    .map(|p| asset_server.load::<Image>(p.clone()));
                let texture_1 = custom_def.textures.get("texture_1")
                    .map(|p| asset_server.load::<Image>(p.clone()));
                let texture_2 = custom_def.textures.get("texture_2")
                    .map(|p| asset_server.load::<Image>(p.clone()));
                let texture_3 = custom_def.textures.get("texture_3")
                    .map(|p| asset_server.load::<Image>(p.clone()));

                let mat = CustomMaterial {
                    uniforms,
                    texture_0,
                    texture_1,
                    texture_2,
                    texture_3,
                    shader,
                    alpha_mode: map_alpha_mode(&def.alpha_mode),
                    double_sided: def.double_sided,
                    unlit: def.unlit,
                };
                BuiltMaterialHandle::Custom(custom_materials.add(mat))
            }
        }
    }

    fn build_standard(
        asset_server: &AssetServer,
        standard_materials: &mut Assets<StandardMaterial>,
        def: &MaterialDef,
        std_def: &crate::schema::material::StandardMaterialDef,
    ) -> Handle<StandardMaterial> {
        let mut material = StandardMaterial {
            base_color: to_bevy_color(std_def.base_color),
            emissive: to_bevy_color(std_def.emissive).into(),
            metallic: std_def.metallic,
            perceptual_roughness: std_def.perceptual_roughness,
            reflectance: std_def.reflectance,
            double_sided: def.double_sided,
            unlit: def.unlit,
            alpha_mode: map_alpha_mode(&def.alpha_mode),
            ..Default::default()
        };

        if let Some(path) = &std_def.base_color_texture {
            material.base_color_texture = Some(asset_server.load(path));
        }
        if let Some(path) = &std_def.normal_map_texture {
            material.normal_map_texture = Some(asset_server.load(path));
        }
        if let Some(path) = &std_def.metallic_roughness_texture {
            material.metallic_roughness_texture = Some(asset_server.load(path));
        }
        if let Some(path) = &std_def.occlusion_texture {
            material.occlusion_texture = Some(asset_server.load(path));
        }
        if let Some(path) = &std_def.emissive_texture {
            material.emissive_texture = Some(asset_server.load(path));
        }

        standard_materials.add(material)
    }
}

// ---------------------------------------------------------------------------
// System: apply material overrides once GLTF children have appeared.
// ---------------------------------------------------------------------------

pub fn apply_material_overrides(
    mut commands: Commands,
    overrides: Query<(Entity, &PendingMaterialOverride)>,
    children: Query<&Children>,
    mesh_query: Query<Entity, With<Mesh3d>>,
    built_materials: Res<BuiltMaterials>,
) {
    for (root_entity, mat_override) in &overrides {
        let Some(built) = built_materials.0.get(&mat_override.0) else {
            warn!("apply_material_overrides: no built material found for key '{}'", mat_override.0);
            continue;
        };

        // Collect mesh descendants (may be empty if GLTF hasn't spawned children yet)
        let mesh_entities: Vec<Entity> = children
            .iter_descendants(root_entity)
            .filter(|e| mesh_query.get(*e).is_ok())
            .collect();

        if mesh_entities.is_empty() { continue; } // Wait until GLTF children appear

        info!(
            "apply_material_overrides: applying '{}' to {} mesh(es) under {:?}",
            mat_override.0, mesh_entities.len(), root_entity
        );

        for mesh_entity in &mesh_entities {
            match built {
                BuiltMaterialHandle::Standard(h) => {
                    commands.entity(*mesh_entity).insert(MeshMaterial3d(h.clone()));
                }
                BuiltMaterialHandle::Terrain(h) => {
                    commands.entity(*mesh_entity)
                        .remove::<MeshMaterial3d<StandardMaterial>>()
                        .insert(MeshMaterial3d(h.clone()));
                }
                BuiltMaterialHandle::Custom(h) => {
                    commands.entity(*mesh_entity)
                        .remove::<MeshMaterial3d<StandardMaterial>>()
                        .insert(MeshMaterial3d(h.clone()));
                }
            }
        }

        commands.entity(root_entity).remove::<PendingMaterialOverride>();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Packs a `CustomMaterialDef`'s `colors` and `floats` maps into the 4 × Vec4
/// uniform buffer.
///
/// Packing order (both maps sorted alphabetically by key):
/// - Colors are packed first, one color per Vec4 slot (params_0, params_1, …)
/// - Floats fill remaining slots, 4 floats per Vec4
///
/// Example — 1 color + 4 floats:
/// ```text
/// params_0 = color_a.rgba      ← 1st color
/// params_1 = (f0, f1, f2, f3)  ← 4 floats
/// params_2 = (0,  0,  0,  0)
/// params_3 = (0,  0,  0,  0)
/// ```
fn pack_custom_uniforms(def: &crate::schema::material::CustomMaterialDef) -> CustomMaterialUniforms {
    let mut params = [Vec4::ZERO; 4];
    let mut next_vec4 = 0usize;

    // Colors first (sorted alphabetically)
    let mut color_keys: Vec<&String> = def.colors.keys().collect();
    color_keys.sort();
    for key in &color_keys {
        if next_vec4 >= 4 { break; }
        let c = def.colors[*key];
        params[next_vec4] = Vec4::new(c.r, c.g, c.b, c.a);
        next_vec4 += 1;
    }

    // Floats fill remaining component slots (sorted alphabetically)
    let mut float_keys: Vec<&String> = def.floats.keys().collect();
    float_keys.sort();
    let mut component = next_vec4 * 4; // start after the color Vec4s
    for key in &float_keys {
        if component >= 16 { break; }
        let vec_idx = component / 4;
        let comp_idx = component % 4;
        let v = def.floats[*key];
        match comp_idx {
            0 => params[vec_idx].x = v,
            1 => params[vec_idx].y = v,
            2 => params[vec_idx].z = v,
            3 => params[vec_idx].w = v,
            _ => unreachable!(),
        }
        component += 1;
    }

    CustomMaterialUniforms {
        params_0: params[0],
        params_1: params[1],
        params_2: params[2],
        params_3: params[3],
    }
}

fn to_bevy_color(c: ColorDef) -> Color {
    Color::srgba(c.r, c.g, c.b, c.a)
}

fn map_alpha_mode(mode: &AlphaModeDef) -> AlphaMode {
    match mode {
        AlphaModeDef::Opaque        => AlphaMode::Opaque,
        AlphaModeDef::Mask(v)       => AlphaMode::Mask(*v),
        AlphaModeDef::Blend         => AlphaMode::Blend,
        AlphaModeDef::Premultiplied => AlphaMode::Premultiplied,
        AlphaModeDef::Add           => AlphaMode::Add,
        AlphaModeDef::Multiply      => AlphaMode::Multiply,
    }
}

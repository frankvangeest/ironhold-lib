use bevy::prelude::*;
use bevy::pbr::StandardMaterial;
use std::collections::HashMap;

use crate::schema::material::{AlphaModeDef, ColorDef, MaterialDef, MaterialKind};
use crate::capabilities::terrain_material::TerrainMaterial;

// ---------------------------------------------------------------------------
// Built material handle — erased enum so we can store Standard and Terrain
// handles in the same map.
// ---------------------------------------------------------------------------

pub enum BuiltMaterialHandle {
    Standard(Handle<StandardMaterial>),
    Terrain(Handle<TerrainMaterial>),
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
        name: &str,
        def: &MaterialDef,
    ) -> BuiltMaterialHandle {
        match &def.kind {
            MaterialKind::Standard(std_def) => {
                let handle = Self::build_standard(asset_server, standard_materials, def, std_def);
                BuiltMaterialHandle::Standard(handle)
            }
            MaterialKind::Terrain(terrain_def) => {
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
                    uv_scale: Vec4::new(50.0, 0.0, 0.0, 0.0),
                    splatmap:  asset_server.load(terrain_def.splatmap.clone()),
                    texture_r: asset_server.load(fallback(0)),
                    texture_g: asset_server.load(fallback(1)),
                    texture_b: asset_server.load(fallback(2)),
                    texture_a: asset_server.load(fallback(3)),
                };
                BuiltMaterialHandle::Terrain(terrain_materials.add(mat))
            }
            MaterialKind::Custom(custom_def) => {
                // Custom shader pipeline not yet fully implemented.
                // Fall back to a StandardMaterial using any available color data.
                warn!(
                    "Material '{}': Custom shader '{:?}' — falling back to StandardMaterial. \
                     Custom shader support is a planned future feature.",
                    name, custom_def.shader
                );
                let base_color = custom_def.colors.get("base_color")
                    .copied()
                    .unwrap_or(ColorDef { r: 1.0, g: 1.0, b: 1.0, a: 1.0 });
                let mat = StandardMaterial {
                    base_color: to_bevy_color(base_color),
                    alpha_mode: map_alpha_mode(&def.alpha_mode),
                    double_sided: def.double_sided,
                    unlit: def.unlit,
                    ..Default::default()
                };
                BuiltMaterialHandle::Standard(standard_materials.add(mat))
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
            }
        }

        commands.entity(root_entity).remove::<PendingMaterialOverride>();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

use bevy::prelude::*;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{
    AsBindGroup, PrimitiveTopology, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError, VertexFormat,
};
use bevy::shader::{Shader, ShaderRef};
use bevy::asset::uuid_handle;
use bevy_mesh::{Indices, MeshVertexAttribute, MeshVertexBufferLayoutRef};

use crate::schema::catalog::{FoliageDef, FoliageMaterialDef};
use crate::runtime::scene_manager::LoadedAssetCatalog;

pub const FOLIAGE_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("666f6c69-6167-4065-8165-666f6c696101");
pub const FOLIAGE_PREPASS_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("666f6c69-6167-4073-8073-707265703101");

// ─── Custom vertex attribute ──────────────────────────────────────────────────

/// Leaf anchor point (cluster-local space).  Stored alongside the corner offset
/// in `ATTRIBUTE_POSITION` so the billboard vertex shader can expand each quad
/// around its anchor rather than around the mesh origin.
pub const ATTRIBUTE_LEAF_CENTER: MeshVertexAttribute = MeshVertexAttribute::new(
    "LeafCenter",
    0x464f4c_4961_6765_01u64,
    VertexFormat::Float32x3,
);

// ─── Material uniforms ────────────────────────────────────────────────────────

/// GPU uniforms for `FoliageMaterial` — 5 × `vec4<f32>` = 80 bytes.
#[derive(ShaderType, Clone, Default)]
pub struct FoliageMaterialParams {
    pub color_highlight: Vec4,
    pub color_midtone:   Vec4,
    pub color_shadow:    Vec4,
    /// xyz = sun direction (world space), w unused.  Updated each frame.
    pub sun_direction:   Vec4,
    /// x = ao_intensity, y = toon_bands (2/3/4), zw unused.
    pub config:          Vec4,
}

// ─── Material ─────────────────────────────────────────────────────────────────

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct FoliageMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub leaf_texture: Handle<Image>,
    #[uniform(2)]
    pub params: FoliageMaterialParams,
}

impl Material for FoliageMaterial {
    fn vertex_shader() -> ShaderRef {
        FOLIAGE_SHADER_HANDLE.into()
    }
    fn fragment_shader() -> ShaderRef {
        FOLIAGE_SHADER_HANDLE.into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        FOLIAGE_PREPASS_SHADER_HANDLE.into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        FOLIAGE_PREPASS_SHADER_HANDLE.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            ATTRIBUTE_LEAF_CENTER.at_shader_location(10),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

// ─── Plugin ───────────────────────────────────────────────────────────────────

pub struct FoliagePlugin;

impl Plugin for FoliagePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<FoliageMaterial>::default())
            .add_systems(Startup, setup_foliage_shaders)
            .add_systems(Update, (foliage_setup_system, foliage_lighting_sync_system));
    }
}

fn setup_foliage_shaders(mut shaders: ResMut<Assets<Shader>>) {
    let _ = shaders.insert(
        &FOLIAGE_SHADER_HANDLE,
        Shader::from_wgsl(
            include_str!("../../../../assets/shared/shaders/foliage.wgsl"),
            "shared/shaders/foliage.wgsl",
        ),
    );
    let _ = shaders.insert(
        &FOLIAGE_PREPASS_SHADER_HANDLE,
        Shader::from_wgsl(
            include_str!("../../../../assets/shared/shaders/foliage_prepass.wgsl"),
            "shared/shaders/foliage_prepass.wgsl",
        ),
    );
}

// ─── Components ───────────────────────────────────────────────────────────────

/// Attached by the scene loader to foliage root entities; consumed by
/// `foliage_setup_system` which builds the cluster meshes and removes it.
#[derive(Component)]
pub struct PendingFoliage(pub FoliageDef);

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Builds leaf-card cluster meshes for every entity that has a `PendingFoliage`
/// component.  Runs each frame in `Update`; completes in one tick per entity
/// once all async assets are ready.
pub fn foliage_setup_system(
    pending: Query<(Entity, &PendingFoliage, &GlobalTransform)>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut foliage_materials: ResMut<Assets<FoliageMaterial>>,
    asset_server: Res<AssetServer>,
    asset_catalog: Res<LoadedAssetCatalog>,
) {
    for (entity, pending_foliage, _gtf) in &pending {
        let def = &pending_foliage.0;

        // Resolve leaf texture path from the asset catalog.
        let Some(texture_path) = asset_catalog.0.textures.get(&def.material.leaf_texture) else {
            warn!(
                "foliage: leaf_texture key '{}' not found in asset catalog; skipping foliage entity (add it to assets.ron textures)",
                def.material.leaf_texture
            );
            commands.entity(entity).remove::<PendingFoliage>();
            continue;
        };
        let leaf_texture: Handle<Image> = asset_server.load(texture_path.clone());

        // Build a shared material handle for all clusters of this tree.
        let mat_handle = foliage_materials.add(make_material(leaf_texture, &def.material));

        // Spawn one child entity per cluster.
        let n_clusters = def.clusters.count as usize;
        for ci in 0..n_clusters {
            let sphere_offset = fibonacci_sphere_biased(
                ci, n_clusters, def.clusters.height_bias, def.clusters.seed,
            ) * def.clusters.emitter_radius;
            let cluster_pos = sphere_offset + Vec3::Y * def.clusters.crown_height;
            let mesh_handle = build_cluster_mesh(ci, n_clusters, def, &mut meshes);
            let mut child_cmd = commands.spawn((
                Mesh3d(mesh_handle),
                MeshMaterial3d(mat_handle.clone()),
                Transform::from_translation(cluster_pos),
            ));
            if !def.cast_shadows {
                child_cmd.insert(bevy::light::NotShadowCaster);
            }
            let child = child_cmd.id();
            commands.entity(entity).add_child(child);
        }

        commands.entity(entity).remove::<PendingFoliage>();
    }
}

/// Keeps the `sun_direction` uniform in every `FoliageMaterial` in sync with
/// the scene's directional light direction.  Runs each frame in `Update`.
pub fn foliage_lighting_sync_system(
    lights: Query<&GlobalTransform, With<DirectionalLight>>,
    mut foliage_materials: ResMut<Assets<FoliageMaterial>>,
) {
    let sun_dir = lights.iter().next()
        .map(|gt| {
            let (_, rot, _) = gt.to_scale_rotation_translation();
            -(rot * Vec3::NEG_Z).normalize()
        })
        .unwrap_or(Vec3::new(-0.5, -1.0, -0.3).normalize());

    for (_, mat) in foliage_materials.iter_mut() {
        if mat.params.sun_direction.xyz() != sun_dir {
            mat.params.sun_direction = sun_dir.extend(0.0);
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn make_material(leaf_texture: Handle<Image>, def: &FoliageMaterialDef) -> FoliageMaterial {
    FoliageMaterial {
        leaf_texture,
        params: FoliageMaterialParams {
            color_highlight: Vec4::new(def.color_highlight.0, def.color_highlight.1, def.color_highlight.2, 1.0),
            color_midtone:   Vec4::new(def.color_midtone.0,   def.color_midtone.1,   def.color_midtone.2,   1.0),
            color_shadow:    Vec4::new(def.color_shadow.0,    def.color_shadow.1,    def.color_shadow.2,    1.0),
            sun_direction:   Vec4::new(-0.5, -1.0, -0.3, 0.0),
            config:          Vec4::new(def.ao_intensity, def.toon_bands as f32, 0.0, 0.0),
        },
    }
}

/// Fibonacci sphere with height bias and seed.
///
/// `height_bias` in [0, 1]: 0.0 = full sphere, 1.0 = upper hemisphere only.
/// `seed` rotates the azimuth pattern so different instances look varied.
fn fibonacci_sphere_biased(i: usize, n: usize, height_bias: f32, seed: u32) -> Vec3 {
    let golden = (1.0 + 5.0f32.sqrt()) / 2.0;
    // Seed shifts the azimuth offset without changing the height distribution.
    let theta = 2.0 * std::f32::consts::PI * (i + seed as usize) as f32 / golden;
    // height_bias maps directly to the lower bound of cos_phi:
    //   0.0 → full sphere (cos_phi in [-1, 1])
    //   0.5 → upper hemisphere (cos_phi in [0, 1], nothing below equator)
    //   1.0 → top only (cos_phi ≈ 1)
    let range   = 2.0 * (1.0 - height_bias.clamp(0.0, 1.0));
    let cos_phi = (1.0 - range * (i as f32 + 0.5) / n.max(1) as f32).clamp(-1.0, 1.0);
    let phi     = cos_phi.acos();
    Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin())
}

/// Builds a single cluster mesh: `leaves_per_cluster` leaf-card quads, each
/// stored as 4 vertices with corner offsets in `ATTRIBUTE_POSITION` and the
/// leaf anchor in `ATTRIBUTE_LEAF_CENTER`.  Sphere normals are baked at
/// build time (leaf anchor normalised relative to cluster centre).
fn build_cluster_mesh(
    cluster_idx: usize,
    cluster_count: usize,
    def: &FoliageDef,
    meshes: &mut Assets<Mesh>,
) -> Handle<Mesh> {
    let n       = def.clusters.leaves_per_cluster as usize;
    let inner_r = def.clusters.emitter_radius * 0.45; // leaf distribution within cluster

    let mut positions:    Vec<[f32; 3]> = Vec::with_capacity(n * 4);
    let mut normals:      Vec<[f32; 3]> = Vec::with_capacity(n * 4);
    let mut uvs:          Vec<[f32; 2]> = Vec::with_capacity(n * 4);
    let mut leaf_centers: Vec<[f32; 3]> = Vec::with_capacity(n * 4);
    let mut indices:      Vec<u32>      = Vec::with_capacity(n * 6);

    // Offset inner Fibonacci per cluster AND per tree seed so same-prefab
    // instances look varied.
    let seed_offset = cluster_idx * n + cluster_count + def.clusters.seed as usize;

    for li in 0..n {
        let leaf_pos     = fibonacci_sphere_biased(seed_offset + li, n * cluster_count, 0.0, 0) * inner_r;
        let sphere_normal = leaf_pos.normalize_or_zero();

        // Scale variation: use golden-ratio scramble for pseudo-random distribution.
        let t  = ((li as f32 + cluster_idx as f32 * 0.618034) * 0.618034).fract();
        let hs = (def.clusters.leaf_scale_min
            + t * (def.clusters.leaf_scale_max - def.clusters.leaf_scale_min)) * 0.5;

        let base = (li * 4) as u32;

        for &(cx, cy) in &[(-hs, hs), (hs, hs), (hs, -hs), (-hs, -hs)] {
            positions.push([cx, cy, 0.0]);
            normals.push(sphere_normal.to_array());
            leaf_centers.push(leaf_pos.to_array());
        }
        uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL,   normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0,     uvs);
    mesh.insert_attribute(ATTRIBUTE_LEAF_CENTER,    leaf_centers);
    mesh.insert_indices(Indices::U32(indices));
    meshes.add(mesh)
}

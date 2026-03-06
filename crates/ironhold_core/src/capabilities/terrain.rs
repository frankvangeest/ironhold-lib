use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy_mesh::Indices;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use futures_lite::future;
use crate::schema::level::TerrainConfig;
use crate::capabilities::terrain_material::TerrainMaterial;
use bevy::render::render_resource::PrimitiveTopology;
use bevy_rapier3d::prelude::*;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<TerrainMaterial>::default());
        app.add_systems(Startup, setup_terrain_shader);
        app.add_systems(Update, (
            terrain_init_system,
            start_terrain_generation_system,
            poll_terrain_generation_system,
        ));
    }
}

#[derive(Component)]
pub struct TerrainLoading {
    pub heightmap_handle: Handle<Image>,
    pub splatmap_handle: Handle<Image>,
    pub material_handles: Vec<Handle<Image>>,
}

#[derive(Component)]
pub struct TerrainGenerationTask(Task<Mesh>);

#[derive(Component)]
pub struct TerrainReady;

fn terrain_init_system(
    mut commands: Commands,
    query: Query<(Entity, &TerrainConfig), (Without<TerrainLoading>, Without<TerrainGenerationTask>, Without<TerrainReady>)>,
    asset_server: Res<AssetServer>,
) {
    for (entity, config) in &query {
        info!("Initializing Terrain: {}", config.heightmap_path);
        let heightmap_handle = asset_server.load(config.heightmap_path.clone());
        let splatmap_handle = asset_server.load(config.splatmap_path.clone());
        let material_handles = config.material_paths.iter()
            .map(|path| asset_server.load(path.clone()))
            .collect();
        
        commands.entity(entity).insert(TerrainLoading {
            heightmap_handle,
            splatmap_handle,
            material_handles,
        });
    }
}

// 1. Detect images loaded -> Spawn Task
fn start_terrain_generation_system(
    mut commands: Commands,
    mut query: Query<(Entity, &TerrainConfig, &TerrainLoading), (Without<TerrainGenerationTask>, Without<TerrainReady>)>,
    images: Res<Assets<Image>>,
) {
    let thread_pool = AsyncComputeTaskPool::get();

    for (entity, config, loading) in &mut query {
        if let Some(heightmap) = images.get(&loading.heightmap_handle) {
            if let Some(data) = &heightmap.data {
                info!("Heightmap loaded ({:?}). Starting async generation...", heightmap.texture_descriptor.size);
                
                // Extract raw data to pass to thread
                let width = heightmap.texture_descriptor.size.width as usize;
                let height = heightmap.texture_descriptor.size.height as usize;
                let data = data.clone(); 
                let height_scale = config.height_scale;
                let horizontal_scale = config.horizontal_scale;

                let task = thread_pool.spawn(async move {
                    info!("Terrain Task Started on Worker Thread");
                    generate_terrain_mesh_raw(width, height, &data, height_scale, horizontal_scale)
                });

                commands.entity(entity).insert(TerrainGenerationTask(task));
                // NOTE: Do NOT remove TerrainLoading here — poll_terrain_generation_system needs it
            }
        }
    }
}

// 2. Poll Task -> Apply Result
fn poll_terrain_generation_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut TerrainGenerationTask, &TerrainConfig, &TerrainLoading)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain_materials: ResMut<Assets<TerrainMaterial>>,
) {
    for (entity, mut task, config, loading) in &mut query {
        if let Some(mesh) = future::block_on(future::poll_once(&mut task.0)) {
            info!("Terrain Generation Completed. Applying to entity.");
            
            let collider = Collider::from_bevy_mesh(&mesh, &ComputedColliderShape::TriMesh(TriMeshFlags::default())).unwrap();
            let mesh_handle = meshes.add(mesh);
            
            // Construct TerrainMaterial using handles from loading
            let terrain_material = TerrainMaterial {
                uv_scale: Vec4::new(10.0, 0.0, 0.0, 0.0), // Only .x is used
                splatmap: loading.splatmap_handle.clone(),
                texture_r: loading.material_handles.get(0).cloned().unwrap_or_default(),
                texture_g: loading.material_handles.get(1).cloned().unwrap_or_default(),
                texture_b: loading.material_handles.get(2).cloned().unwrap_or_default(),
                texture_a: loading.material_handles.get(3).cloned().unwrap_or_default(),
            };
            let material_handle = terrain_materials.add(terrain_material);

            let (px, py, pz) = config.position;

            commands.entity(entity).insert((
                Mesh3d(mesh_handle),
                MeshMaterial3d(material_handle),
                Transform::from_xyz(px, py, pz),
                Visibility::default(),
                TerrainReady,
                RigidBody::Fixed,
                collider,
            ));
            
            commands.entity(entity).remove::<TerrainGenerationTask>();
            commands.entity(entity).remove::<TerrainLoading>();
        }
    }
}

// Pure function, no bevy types except Mesh output
fn generate_terrain_mesh_raw(width: usize, height: usize, data: &[u8], height_scale: f32, horizontal_scale: f32) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    
    let num_verts = width * height;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(num_verts);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(num_verts);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(num_verts);
    
    let extract_height = |x: usize, z: usize| -> f32 {
        let idx = (z * width + x) * 4; // Assuming RGBA8
        if idx < data.len() {
            (data[idx] as f32 / 255.0) * height_scale
        } else {
            0.0
        }
    };

    // Center offset
    let offset_x = (width - 1) as f32 * horizontal_scale * 0.5;
    let offset_z = (height - 1) as f32 * horizontal_scale * 0.5;

    // 1. Generate Vertices
    for z in 0..height {
        for x in 0..width {
            let y = extract_height(x, z);
            let vx = x as f32 * horizontal_scale - offset_x;
            let vz = z as f32 * horizontal_scale - offset_z;

            positions.push([vx, y, vz]);
            normals.push([0.0, 1.0, 0.0]); // Placeholder, compute_smooth_normals will fix this
            uvs.push([x as f32 / (width - 1) as f32, z as f32 / (height - 1) as f32]);
        }
    }

    // 2. Generate Indices
    let mut indices = Vec::with_capacity((width - 1) * (height - 1) * 6);
    for z in 0..height - 1 {
        for x in 0..width - 1 {
            let i00 = z * width + x;
            let i10 = z * width + (x + 1);
            let i01 = (z + 1) * width + x;
            let i11 = (z + 1) * width + (x + 1);

            // Tri 1
            indices.push(i00 as u32);
            indices.push(i10 as u32);
            indices.push(i01 as u32);

            // Tri 2
            indices.push(i10 as u32);
            indices.push(i11 as u32);
            indices.push(i01 as u32);
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    
    mesh.compute_smooth_normals();
    
    mesh
}

// Inject the embedded shader so WebGPU doesn't rely on runtime asset loading for it
fn setup_terrain_shader(mut shaders: ResMut<Assets<Shader>>) {
    let shader = bevy::shader::Shader::from_wgsl(
        include_str!("../../../../assets/shaders/terrain.wgsl"),
        "shaders/terrain.wgsl"
    );
    let _ = shaders.insert(&crate::capabilities::terrain_material::TERRAIN_SHADER_HANDLE, shader);
}

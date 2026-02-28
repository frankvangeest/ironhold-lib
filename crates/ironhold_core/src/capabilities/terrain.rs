use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use futures_lite::future;
use crate::schema::level::TerrainConfig;
use crate::capabilities::terrain_material::TerrainMaterial;
use bevy::render::render_resource::PrimitiveTopology;

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
            ));
            
            commands.entity(entity).remove::<TerrainGenerationTask>();
            commands.entity(entity).remove::<TerrainLoading>();
        }
    }
}

// Pure function, no bevy types except Mesh output
fn generate_terrain_mesh_raw(width: usize, height: usize, data: &[u8], height_scale: f32, horizontal_scale: f32) -> Mesh {
    // Create a mesh
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    
    // Pre-allocate
    let num_quads = (width - 1) * (height - 1);
    let num_verts = num_quads * 6;
    
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

    // Non-indexed geometry
    for z in 0..height-1 {
        for x in 0..width-1 {
            let y00 = extract_height(x, z);
            let y10 = extract_height(x+1, z);
            let y01 = extract_height(x, z+1);
            let y11 = extract_height(x+1, z+1);
            
            let vx0 = x as f32 * horizontal_scale - offset_x;
            let vx1 = (x + 1) as f32 * horizontal_scale - offset_x;
            let vz0 = z as f32 * horizontal_scale - offset_z;
            let vz1 = (z + 1) as f32 * horizontal_scale - offset_z;

            // Tri 1
            positions.push([vx0, y00, vz0]);
            uvs.push([x as f32 / width as f32, z as f32 / height as f32]);
            normals.push([0.0, 1.0, 0.0]);

            positions.push([vx1, y10, vz0]);
            uvs.push([(x+1) as f32 / width as f32, z as f32 / height as f32]);
            normals.push([0.0, 1.0, 0.0]);

            positions.push([vx0, y01, vz1]);
            uvs.push([x as f32 / width as f32, (z+1) as f32 / height as f32]);
            normals.push([0.0, 1.0, 0.0]);

            // Tri 2
            positions.push([vx1, y10, vz0]);
            uvs.push([(x+1) as f32 / width as f32, z as f32 / height as f32]);
            normals.push([0.0, 1.0, 0.0]);

            positions.push([vx1, y11, vz1]);
            uvs.push([(x+1) as f32 / width as f32, (z+1) as f32 / height as f32]);
            normals.push([0.0, 1.0, 0.0]);

            positions.push([vx0, y01, vz1]);
            uvs.push([x as f32 / width as f32, (z+1) as f32 / height as f32]);
            normals.push([0.0, 1.0, 0.0]);
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    
    mesh.compute_flat_normals();
    
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

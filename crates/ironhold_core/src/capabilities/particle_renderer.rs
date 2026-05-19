// capabilities/particle_renderer.rs
//
// Pool-based particle renderer.  All particles of the same (blend_mode, texture) share
// ONE mesh entity rebuilt each frame — giving O(distinct textures) draw calls instead
// of O(particle count).
//
// Architecture
// ───────────
//   • `ParticlePool`      — flat Vec of CPU particle states (no ECS entity per particle).
//   • `ParticlePoolGroups`— one mesh entity + shared material per (blend_mode, texture_path).
//   • `simulate_pool_system`      — physics tick (gravity, turbulence, velocity, size).
//   • `rebuild_pool_meshes_system`— CPU billboard computation, mesh upload each frame.
//   • `clear_pool_on_scene_unload_system` — reset on SceneEvent::Unloading.
//
// Flame distort particles (uv_distort > 0 || uv_scroll_speed > 0) use a separate
// `PoolFlameMaterial` that reads per-particle elapsed time from mesh UV1 (uv_b.x),
// so all flame particles of the same texture still share one material handle.

use bevy::prelude::*;
use bevy_mesh::Indices;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
    PrimitiveTopology,
};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::shader::ShaderRef;
use bevy_mesh::MeshVertexBufferLayoutRef;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::asset::RenderAssetUsages;
use std::collections::HashMap;
use crate::runtime::scene_manager::LevelEntity;
use crate::runtime::messages::SceneEvent;

// ─── Pool particle state ──────────────────────────────────────────────────────

pub struct PooledParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub elapsed: f32,
    pub duration: f32,
    pub start_size: f32,
    pub end_size: Option<f32>,
    pub gravity: f32,
    pub turbulence: f32,
    pub noise_seed: f32,
    pub color_start: LinearRgba,
    pub color_mid: Option<LinearRgba>,
    pub color_end: LinearRgba,
    pub is_additive: bool,
    /// Resolved asset path (e.g. `"particle/smoke.png"`). Empty = untextured sphere-like.
    pub texture_path: String,
    /// UV scroll speed for flame distort variant. 0 = standard material.
    pub uv_scroll_speed: f32,
    /// UV distort strength for flame distort variant. 0 = standard material.
    pub uv_distort: f32,
}

impl PooledParticle {
    pub fn is_alive(&self) -> bool {
        self.elapsed < self.duration
    }
    pub fn is_flame(&self) -> bool {
        self.uv_scroll_speed > 0.0 || self.uv_distort > 0.0
    }
    pub fn group_key(&self) -> GroupKey {
        if self.is_flame() {
            GroupKey::Flame {
                scroll_k: (self.uv_scroll_speed * 1000.0) as u32,
                distort_k: (self.uv_distort * 1000.0) as u32,
                texture_path: self.texture_path.clone(),
            }
        } else if self.is_additive {
            GroupKey::Additive { texture_path: self.texture_path.clone() }
        } else {
            GroupKey::Blend { texture_path: self.texture_path.clone() }
        }
    }
}

// ─── Resources ───────────────────────────────────────────────────────────────

/// Flat CPU particle pool.  Dead slots (elapsed >= duration) are reused on alloc.
#[derive(Resource, Default)]
pub struct ParticlePool {
    pub particles: Vec<PooledParticle>,
}

impl ParticlePool {
    /// Insert a new particle, reusing the first dead slot if available.
    pub fn alloc(&mut self, p: PooledParticle) {
        if let Some(slot) = self.particles.iter().position(|q| !q.is_alive()) {
            self.particles[slot] = p;
        } else {
            self.particles.push(p);
        }
    }
}

/// One mesh entity per render group.  Created lazily; cleared on scene unload.
struct PoolGroup {
    mesh_handle: Handle<Mesh>,
}

/// Identifies a render group — same key → same draw call.
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub enum GroupKey {
    Additive { texture_path: String },
    Blend    { texture_path: String },
    Flame    { scroll_k: u32, distort_k: u32, texture_path: String },
}

/// Registry of live render groups + cached material handles.
/// Material handles persist across scenes (not LevelEntity).
/// Group entity IDs are cleared on SceneEvent::Unloading (entities are LevelEntity, auto-despawned).
#[derive(Resource, Default)]
pub struct ParticlePoolGroups {
    groups: HashMap<GroupKey, PoolGroup>,
    std_mats: HashMap<(bool, String), Handle<StandardMaterial>>, // (is_additive, texture_path)
    flame_mats: HashMap<(u32, u32, String), Handle<PoolFlameMaterial>>,
}

// ─── Custom flame material ────────────────────────────────────────────────────

/// Per-effect constants for UV animation.  Per-particle elapsed is encoded in mesh UV1.
#[derive(ShaderType, Clone, Default)]
pub struct PoolFlameUniforms {
    /// (scroll_speed, distort_strength, unused, unused).
    pub params: Vec4,
}

/// Shared material for all flame particles with the same scroll/distort/texture.
/// Reads `elapsed` from vertex UV1 (`in.uv_b.x`) so all particles share one handle.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct PoolFlameMaterial {
    #[uniform(0)]
    pub uniforms: PoolFlameUniforms,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
}

impl Material for PoolFlameMaterial {
    fn fragment_shader() -> ShaderRef {
        "shared/shaders/pool_flame_particle.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Tick all alive particles: gravity, turbulence, velocity integration, size lerp.
/// Does NOT update material uniforms or transforms (those come from mesh rebuild).
pub fn simulate_pool_system(mut pool: ResMut<ParticlePool>, time: Res<Time>) {
    let dt = time.delta_secs();
    for p in pool.particles.iter_mut() {
        if !p.is_alive() { continue; }
        p.elapsed += dt;
        p.velocity.y += p.gravity * dt;
        if p.turbulence > 0.0 {
            let s = p.noise_seed;
            let te = p.elapsed;
            let dx = ((te * 3.1 + s).sin() + (te * 1.7 + s * 2.3).sin() * 0.5) * p.turbulence;
            let dz = ((te * 2.7 + s * 1.5).cos() + (te * 4.1 + s * 0.8).cos() * 0.5) * p.turbulence;
            p.velocity.x += dx * dt;
            p.velocity.z += dz * dt;
        }
        p.position += p.velocity * dt;
    }
}

/// Rebuild mesh vertices for every active render group each frame.
/// Creates group entities lazily on first use; zeroes meshes for empty groups.
pub fn rebuild_pool_meshes_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut std_materials: ResMut<Assets<StandardMaterial>>,
    mut flame_materials: ResMut<Assets<PoolFlameMaterial>>,
    mut groups: ResMut<ParticlePoolGroups>,
    pool: Res<ParticlePool>,
    camera_q: Query<&GlobalTransform, With<Camera3d>>,
    asset_server: Res<AssetServer>,
) {
    // Camera basis vectors for billboard orientation.
    let (cam_right, cam_up) = if let Ok(cam_gt) = camera_q.single() {
        (cam_gt.right().as_vec3(), cam_gt.up().as_vec3())
    } else {
        (Vec3::X, Vec3::Y)
    };

    // Bucket alive particles by group key.
    let mut buckets: HashMap<GroupKey, Vec<usize>> = HashMap::new();
    for (idx, p) in pool.particles.iter().enumerate() {
        if p.is_alive() {
            buckets.entry(p.group_key()).or_default().push(idx);
        }
    }

    // Ensure a group entity + material exist for every active bucket.
    for key in buckets.keys() {
        if groups.groups.contains_key(key) { continue; }

        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        let mesh_handle = meshes.add(mesh);

        match key {
            GroupKey::Additive { texture_path } | GroupKey::Blend { texture_path } => {
                let is_add = matches!(key, GroupKey::Additive { .. });
                let mat_handle = groups.std_mats
                    .entry((is_add, texture_path.clone()))
                    .or_insert_with(|| {
                        let tex = if texture_path.is_empty() {
                            None
                        } else {
                            Some(asset_server.load(texture_path.clone()))
                        };
                        std_materials.add(StandardMaterial {
                            base_color: Color::WHITE,
                            base_color_texture: tex,
                            unlit: true,
                            alpha_mode: if is_add { AlphaMode::Add } else { AlphaMode::Blend },
                            double_sided: true,
                            cull_mode: None,
                            ..default()
                        })
                    })
                    .clone();
                commands.spawn((
                    Mesh3d(mesh_handle.clone()),
                    MeshMaterial3d(mat_handle),
                    Transform::default(),
                    Visibility::default(),
                    LevelEntity,
                    NoFrustumCulling,
                ));
            }
            GroupKey::Flame { scroll_k, distort_k, texture_path } => {
                let mat_handle = groups.flame_mats
                    .entry((*scroll_k, *distort_k, texture_path.clone()))
                    .or_insert_with(|| {
                        let scroll = *scroll_k as f32 / 1000.0;
                        let distort = *distort_k as f32 / 1000.0;
                        let tex = if texture_path.is_empty() {
                            None
                        } else {
                            Some(asset_server.load(texture_path.clone()))
                        };
                        flame_materials.add(PoolFlameMaterial {
                            uniforms: PoolFlameUniforms {
                                params: Vec4::new(scroll, distort, 0.0, 0.0),
                            },
                            texture: tex,
                        })
                    })
                    .clone();
                commands.spawn((
                    Mesh3d(mesh_handle.clone()),
                    MeshMaterial3d(mat_handle),
                    Transform::default(),
                    Visibility::default(),
                    LevelEntity,
                    NoFrustumCulling,
                ));
            }
        }

        groups.groups.insert(key.clone(), PoolGroup { mesh_handle });
    }

    // Rebuild mesh for each group.
    let all_keys: Vec<GroupKey> = groups.groups.keys().cloned().collect();
    for key in &all_keys {
        let Some(group) = groups.groups.get(key) else { continue };
        let Some(mesh) = meshes.get_mut(&group.mesh_handle) else { continue };
        let is_flame = matches!(key, GroupKey::Flame { .. });

        let indices_ref = buckets.get(key);
        if indices_ref.map_or(true, |v| v.is_empty()) {
            // Zero out this group's mesh so no stale geometry renders.
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
            if is_flame {
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, Vec::<[f32; 2]>::new());
            }
            mesh.insert_indices(Indices::U32(vec![]));
            continue;
        }
        let particle_indices = indices_ref.unwrap();
        let n = particle_indices.len();

        let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n * 4);
        let mut uvs:       Vec<[f32; 2]> = Vec::with_capacity(n * 4);
        let mut colors:    Vec<[f32; 4]> = Vec::with_capacity(n * 4);
        let mut uv1:       Vec<[f32; 2]> = if is_flame { Vec::with_capacity(n * 4) } else { Vec::new() };
        let mut idx_buf:   Vec<u32>       = Vec::with_capacity(n * 6);

        for (qi, &pi) in particle_indices.iter().enumerate() {
            let p = &pool.particles[pi];
            let t = (p.elapsed / p.duration).min(1.0);
            let color = lerp_color(p, t);
            let size  = lerp_size(p, t);
            let c = [color.red, color.green, color.blue, color.alpha];

            let half = size * 0.5;
            let r = cam_right * half;
            let u = cam_up  * half;
            let pos = p.position;

            // Bottom-left, bottom-right, top-right, top-left.
            let v0: [f32; 3] = (pos - r - u).into();
            let v1: [f32; 3] = (pos + r - u).into();
            let v2: [f32; 3] = (pos + r + u).into();
            let v3: [f32; 3] = (pos - r + u).into();

            positions.extend_from_slice(&[v0, v1, v2, v3]);
            // UV: tip at y=0, base at y=1 (matches Kenney flame sprite orientation).
            uvs.extend_from_slice(&[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
            colors.extend_from_slice(&[c, c, c, c]);

            if is_flame {
                let e = p.elapsed;
                uv1.extend_from_slice(&[[e, 0.0], [e, 0.0], [e, 0.0], [e, 0.0]]);
            }

            let b = (qi * 4) as u32;
            idx_buf.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        if is_flame {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uv1);
        }
        mesh.insert_indices(Indices::U32(idx_buf));
    }
}

/// Clear the pool and group-entity registry on full scene replacement so no stale
/// particles carry over.  Group materials persist (they are not LevelEntity).
pub fn clear_pool_on_scene_unload_system(
    mut pool: ResMut<ParticlePool>,
    mut groups: ResMut<ParticlePoolGroups>,
    mut scene_events: MessageReader<SceneEvent>,
) {
    for event in scene_events.read() {
        if matches!(event, SceneEvent::Unloading(_)) {
            pool.particles.clear();
            groups.groups.clear(); // entity IDs invalid after LevelEntity despawn
        }
    }
}

// ─── Colour / size helpers ─────────────────────────────────────────────────────

fn lerp_rgba(a: LinearRgba, b: LinearRgba, t: f32) -> LinearRgba {
    LinearRgba {
        red:   a.red   + (b.red   - a.red)   * t,
        green: a.green + (b.green - a.green) * t,
        blue:  a.blue  + (b.blue  - a.blue)  * t,
        alpha: a.alpha + (b.alpha - a.alpha) * t,
    }
}

pub fn lerp_color(p: &PooledParticle, t: f32) -> LinearRgba {
    let (cs, ce) = (p.color_start, p.color_end);
    if let Some(cm) = p.color_mid {
        if t < 0.5 { lerp_rgba(cs, cm, t * 2.0) }
        else       { lerp_rgba(cm, ce, (t - 0.5) * 2.0) }
    } else {
        lerp_rgba(cs, ce, t)
    }
}

pub fn lerp_size(p: &PooledParticle, t: f32) -> f32 {
    if let Some(end) = p.end_size {
        p.start_size + (end - p.start_size) * t
    } else {
        p.start_size
    }
}

// ─── Plugin ───────────────────────────────────────────────────────────────────

pub struct ParticleRendererPlugin;

impl Plugin for ParticleRendererPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticlePool>()
            .init_resource::<ParticlePoolGroups>()
            .add_plugins(MaterialPlugin::<PoolFlameMaterial>::default());
    }
}

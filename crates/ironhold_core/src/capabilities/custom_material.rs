use bevy::asset::uuid_handle;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

/// UUID handle for the built-in fallback shader that renders magenta to signal
/// that a `CustomMaterial` was created without a valid `shader` path.
pub const CUSTOM_MATERIAL_FALLBACK_HANDLE: Handle<Shader> =
    uuid_handle!("63757374-6f6d-4d61-7400-000000000001");

// ---------------------------------------------------------------------------
// Uniform layout (binding 0)
// ---------------------------------------------------------------------------

/// Uniform data for `CustomMaterial` — 4 × `vec4<f32>` = 64 bytes.
///
/// Packing convention (both maps sorted alphabetically by key):
/// - `colors` entries (each → one `vec4<f32>`) fill `params_0`, `params_1`, …
/// - `floats` entries (packed 4 per `vec4`) fill the remaining slots
///
/// Example with 1 color ("base_color") + 3 floats ("roughness", "tiling", "x"):
/// ```text
/// params_0 = base_color.rgba
/// params_1 = (roughness, tiling, x, 0.0)
/// params_2 = (0, 0, 0, 0)
/// params_3 = (0, 0, 0, 0)
/// ```
#[derive(ShaderType, Debug, Clone, Copy, Default)]
pub struct CustomMaterialUniforms {
    pub params_0: Vec4,
    pub params_1: Vec4,
    pub params_2: Vec4,
    pub params_3: Vec4,
}

// ---------------------------------------------------------------------------
// Pipeline key — carries the per-instance shader for specialization
// ---------------------------------------------------------------------------

/// Pipeline key that identifies which shader a `CustomMaterial` instance uses.
/// Two materials with the same shader share a GPU pipeline; different shaders
/// get separate pipelines.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CustomMaterialKey {
    pub shader: Handle<Shader>,
}

impl From<&CustomMaterial> for CustomMaterialKey {
    fn from(mat: &CustomMaterial) -> Self {
        Self { shader: mat.shader.clone() }
    }
}

// ---------------------------------------------------------------------------
// CustomMaterial asset
// ---------------------------------------------------------------------------

/// A fully data-driven Bevy `Material` that uses a designer-supplied WGSL
/// fragment shader.
///
/// ## WGSL interface contract
///
/// Your shader must declare the following bindings in the engine's material
/// bind group (`#{MATERIAL_BIND_GROUP}`):
///
/// ```wgsl
/// struct CustomMaterialUniforms {
///     params_0: vec4<f32>,
///     params_1: vec4<f32>,
///     params_2: vec4<f32>,
///     params_3: vec4<f32>,
/// }
/// @group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CustomMaterialUniforms;
/// @group(#{MATERIAL_BIND_GROUP}) @binding(1) var texture_0: texture_2d<f32>;
/// @group(#{MATERIAL_BIND_GROUP}) @binding(2) var sampler_0: sampler;
/// @group(#{MATERIAL_BIND_GROUP}) @binding(3) var texture_1: texture_2d<f32>;
/// @group(#{MATERIAL_BIND_GROUP}) @binding(4) var sampler_1: sampler;
/// @group(#{MATERIAL_BIND_GROUP}) @binding(5) var texture_2: texture_2d<f32>;
/// @group(#{MATERIAL_BIND_GROUP}) @binding(6) var sampler_2: sampler;
/// @group(#{MATERIAL_BIND_GROUP}) @binding(7) var texture_3: texture_2d<f32>;
/// @group(#{MATERIAL_BIND_GROUP}) @binding(8) var sampler_3: sampler;
/// ```
///
/// Unused texture slots receive a 1×1 white fallback image and can be ignored.
///
/// ## RON authoring
///
/// ```ron
/// "my_mat": (
///   kind: Custom((
///     shader: Some("shared/shaders/my_effect.wgsl"),
///     textures: { "texture_0": "textures/albedo.png" },
///     colors:  { "base_color": (r:1.0, g:0.8, b:0.6, a:1.0) },
///     floats:  { "roughness": 0.7 },
///   )),
///   alpha_mode: Opaque,
/// )
/// ```
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(CustomMaterialKey)]
pub struct CustomMaterial {
    // ── Uniform buffer ────────────────────────────────────────────────────────
    #[uniform(0)]
    pub uniforms: CustomMaterialUniforms,

    // ── Texture slots ─────────────────────────────────────────────────────────
    #[texture(1)]
    #[sampler(2)]
    pub texture_0: Option<Handle<Image>>,

    #[texture(3)]
    #[sampler(4)]
    pub texture_1: Option<Handle<Image>>,

    #[texture(5)]
    #[sampler(6)]
    pub texture_2: Option<Handle<Image>>,

    #[texture(7)]
    #[sampler(8)]
    pub texture_3: Option<Handle<Image>>,

    // ── Non-binding fields (not part of the bind group) ───────────────────────
    /// Handle to the user's WGSL fragment shader.
    /// Defaults to `CUSTOM_MATERIAL_FALLBACK_HANDLE` (renders magenta).
    pub shader: Handle<Shader>,

    pub alpha_mode: AlphaMode,
    pub double_sided: bool,
    pub unlit: bool,
}

impl Default for CustomMaterial {
    fn default() -> Self {
        Self {
            uniforms: CustomMaterialUniforms::default(),
            texture_0: None,
            texture_1: None,
            texture_2: None,
            texture_3: None,
            shader: CUSTOM_MATERIAL_FALLBACK_HANDLE,
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
            unlit: false,
        }
    }
}

impl Material for CustomMaterial {
    /// Returns the fallback magenta shader by default.
    /// `specialize()` overrides this per-pipeline when the material carries a
    /// non-fallback shader handle.
    fn fragment_shader() -> ShaderRef {
        CUSTOM_MATERIAL_FALLBACK_HANDLE.into()
    }

    /// Swap in the per-instance shader when it differs from the built-in fallback.
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &bevy_mesh::MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.shader.id() != CUSTOM_MATERIAL_FALLBACK_HANDLE.id() {
            if let Some(frag) = descriptor.fragment.as_mut() {
                // Only swap in the forward pass. The prepass uses a different
                // vertex output layout (Float32x2 at loc 0 vs Float32x4),
                // and WebGPU strictly validates the interface — do not override it.
                if frag.shader.id() == CUSTOM_MATERIAL_FALLBACK_HANDLE.id() {
                    frag.shader = key.bind_group_data.shader.clone();
                }
            }
        }
        Ok(())
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct CustomMaterialPlugin;

impl Plugin for CustomMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<CustomMaterial>::default());
        app.add_systems(Startup, setup_custom_material_fallback_shader);
    }
}

/// Registers the magenta fallback shader so it is available immediately
/// without relying on the asset server (important for WASM builds).
fn setup_custom_material_fallback_shader(mut shaders: ResMut<Assets<Shader>>) {
    let shader = bevy::shader::Shader::from_wgsl(
        include_str!("../../../../assets/shared/shaders/custom_material_default.wgsl"),
        "shared/shaders/custom_material_default.wgsl",
    );
    let _ = shaders.insert(&CUSTOM_MATERIAL_FALLBACK_HANDLE, shader);
}

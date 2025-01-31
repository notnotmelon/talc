use bevy::render::mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef};
use bevy::render::render_resource::{
    PolygonMode, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError, VertexFormat,
};
use bevy::{
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::AsBindGroup,
};

use crate::chunk::Chunk;

#[derive(Resource)]
pub enum ChunkMaterialWireframeMode {
    On,
    Off,
}

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ChunkMaterial>::default());
        app.add_plugins(MaterialPlugin::<ChunkMaterialWireframe>::default());
        app.insert_resource(ChunkMaterialWireframeMode::Off);
        app.add_systems(Update, apply_chunk_material);
    }
}

// This system toggles between wireframe rendering mode and solid rendering mode when the T key is pressed.
#[allow(clippy::needless_pass_by_value)]
fn apply_chunk_material(
    without_wireframe: Query<(Entity, &MeshMaterial3d<ChunkMaterial>), With<Chunk>>,
    with_wireframe: Query<(Entity, &MeshMaterial3d<ChunkMaterialWireframe>), With<Chunk>>,
    input: Res<ButtonInput<KeyCode>>,
    global_chunk_material: Res<GlobalChunkMaterial>,
    global_chunk_wireframe_material: Res<GlobalChunkWireframeMaterial>,
    mut mode: ResMut<ChunkMaterialWireframeMode>,
    mut commands: Commands,
) {
    use ChunkMaterialWireframeMode as F;
    if !input.just_pressed(KeyCode::KeyT) {
        return;
    }

    *mode = match *mode {
        F::On => F::Off,
        F::Off => F::On,
    };
    match *mode {
        F::On => {
            for (entity, _) in without_wireframe.iter() {
                commands
                    .entity(entity)
                    .remove::<MeshMaterial3d<ChunkMaterial>>()
                    .insert(MeshMaterial3d(global_chunk_wireframe_material.0.clone()));
            }
        }
        F::Off => {
            for (entity, _) in with_wireframe.iter() {
                commands
                    .entity(entity)
                    .remove::<MeshMaterial3d<ChunkMaterial>>()
                    .insert(MeshMaterial3d(global_chunk_material.0.clone()));
            }
        }
    }
}

#[derive(Resource, Reflect)]
pub struct GlobalChunkMaterial(pub Handle<ChunkMaterial>);
#[derive(Resource, Reflect)]
pub struct GlobalChunkWireframeMaterial(pub Handle<ChunkMaterialWireframe>);

// A "high" random id should be used for custom attributes to ensure consistent sorting and avoid collisions with other attributes.
// See the MeshVertexAttribute docs for more info.
pub const ATTRIBUTE_VOXEL: MeshVertexAttribute =
    MeshVertexAttribute::new("Voxel", 988540919, VertexFormat::Uint32);

// This is the struct that will be passed to your shader
#[derive(Asset, Reflect, AsBindGroup, Debug, Clone)]
pub struct ChunkMaterial {
    #[uniform(0)]
    pub reflectance: f32,
    #[uniform(0)]
    pub perceptual_roughness: f32,
    #[uniform(0)]
    pub metallic: f32,
}

impl Material for ChunkMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/chunk.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/chunk.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout
            .0
            .get_layout(&[ATTRIBUTE_VOXEL.at_shader_location(0)])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        Ok(())
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/chunk_prepass.wgsl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/chunk_prepass.wgsl".into()
    }
}

// copy of chunk material pipeline but with wireframe
#[derive(Asset, Reflect, AsBindGroup, Debug, Clone)]
pub struct ChunkMaterialWireframe {
    #[uniform(0)]
    pub reflectance: f32,
    #[uniform(0)]
    pub perceptual_roughness: f32,
    #[uniform(0)]
    pub metallic: f32,
}

impl Material for ChunkMaterialWireframe {
    fn vertex_shader() -> ShaderRef {
        "shaders/chunk.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/chunk.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout
            .0
            .get_layout(&[ATTRIBUTE_VOXEL.at_shader_location(0)])?;
        descriptor.primitive.polygon_mode = PolygonMode::Line;
        descriptor.vertex.buffers = vec![vertex_layout];
        Ok(())
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/chunk_prepass.wgsl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/chunk_prepass.wgsl".into()
    }
}

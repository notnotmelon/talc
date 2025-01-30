use std::f32::consts::PI;

use bevy::{app::TaskPoolThreadAssignmentPolicy, core_pipeline::{bloom::Bloom, tonemapping::Tonemapping}, pbr::{Atmosphere, AtmosphereSettings}, render::{
    settings::{RenderCreation, WgpuFeatures, WgpuSettings}, RenderPlugin
}};
use bevy::prelude::*;

use talc::{
    chunk::{CHUNK_SIZE_I32, CHUNK_SIZE2},
    position::FloatingPosition,
    rendering::{
        ChunkMaterial, ChunkMaterialWireframe, GlobalChunkMaterial, GlobalChunkWireframeMaterial,
        RenderingPlugin,
    },
};
use talc::{
    position::RelativePosition,
    scanner::{Scanner, ScannerPlugin},
    sun::SunPlugin,
    voxel::BlockType,
    voxel_engine::{ChunkModification, VoxelEngine, VoxelEnginePlugin},
};

use rand::Rng;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    // WARN this is a native only feature. It will not work with webgl or webgpu
                    features: WgpuFeatures::POLYGON_MODE_LINE,
                    ..default()
                }),
                ..default()
            })
            .set(TaskPoolPlugin {
                task_pool_options: TaskPoolOptions {
                    async_compute: TaskPoolThreadAssignmentPolicy {
                        min_threads: 1,
                        max_threads: 8,
                        percent: 0.75,
                        on_thread_spawn: None,
                        on_thread_destroy: None,
                    },
                    ..default()
                },
            }),))
        .add_plugins(VoxelEnginePlugin)
        .add_plugins(SunPlugin)
        .add_plugins(ScannerPlugin)
        .add_systems(Startup, setup)
        // camera plugin
        .add_plugins(RenderingPlugin)
        .add_systems(Update, modify_current_terrain)
        .run();
}

#[allow(clippy::needless_pass_by_value)]
pub fn modify_current_terrain(
    query: Query<&Transform, With<Camera>>,
    key: Res<ButtonInput<KeyCode>>,
    mut voxel_engine: ResMut<VoxelEngine>,
) {
    if !key.pressed(KeyCode::KeyN) {
        return;
    }
    let camera_transform = query.single();
    let looking_at_position = camera_transform.translation + (camera_transform.forward() * 64.0);
    let looking_at_position = FloatingPosition(looking_at_position);
    let camera_chunk = looking_at_position.into();

    let mut rng = rand::rng();
    let mut mods = vec![];
    for _ in 0..CHUNK_SIZE2 {
        let pos = RelativePosition::new(
            rng.random_range(0..CHUNK_SIZE_I32),
            rng.random_range(0..CHUNK_SIZE_I32),
            rng.random_range(0..CHUNK_SIZE_I32),
        );
        mods.push(ChunkModification(pos, BlockType::Air));
    }
    voxel_engine.chunk_modifications.insert(camera_chunk, mods);
}

pub fn setup(
    mut commands: Commands,
    mut chunk_materials: ResMut<Assets<ChunkMaterial>>,
    mut chunk_materials_wireframe: ResMut<Assets<ChunkMaterialWireframe>>,
    #[allow(unused)] mut materials: ResMut<Assets<StandardMaterial>>,
    #[allow(unused)] mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.spawn((
        Name::new("Sun"),
        talc::sun::Sun,
        DirectionalLight {
            illuminance: light_consts::lux::RAW_SUNLIGHT,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, PI / 2., -PI / 4.)),
    ));

    commands
        .spawn((
            Scanner::new(12),
            Transform::from_xyz(0.0, 2.0, 0.5),
            Camera3d::default(),
            Camera {
                hdr: true,
                ..default()
            },
            Atmosphere::EARTH,
            AtmosphereSettings {
                aerial_view_lut_max_distance: 3.2e5,
                scene_units_to_m: 1e+4,
                ..Default::default()
            },
            Tonemapping::AcesFitted,
            Bloom::NATURAL,
        ));

    commands.insert_resource(GlobalChunkMaterial(chunk_materials.add(ChunkMaterial {
        reflectance: 0.5,
        perceptual_roughness: 1.0,
        metallic: 0.01,
    })));
    commands.insert_resource(GlobalChunkWireframeMaterial(chunk_materials_wireframe.add(
        ChunkMaterialWireframe {
            reflectance: 0.5,
            perceptual_roughness: 1.0,
            metallic: 0.01,
        },
    )));
}

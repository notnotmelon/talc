use std::sync::Arc;

use bevy::{
    asset::LoadState,
    platform_support::collections::{HashMap, HashSet},
    prelude::*,
    render::{
        mesh::Indices, primitives::Aabb, render_asset::RenderAssetUsages,
        render_resource::PrimitiveTopology,
    },
    tasks::{AsyncComputeTaskPool, Task, block_on},
};

use crate::chunky::{
    chunk::{
        CHUNK_FLOAT_UP_BLOCKS_PER_SECOND, CHUNK_INITIAL_Y_OFFSET, CHUNK_SIZE_F32, CHUNK_SIZE_I32,
        Chunk, ChunkData,
    },
    chunk_mesh::ChunkMesh,
    chunks_refs::ChunksRefs,
    greedy_mesher_optimized,
    lod::Lod,
};
use crate::mod_manager::prototypes::{BlockPrototype, BlockPrototypes};
use crate::rendering::{ATTRIBUTE_VOXEL, GlobalChunkMaterial};
use crate::position::{ChunkPosition, FloatingPosition, Position, RelativePosition};
use crate::{
    player::render_distance::Scanner,
    smooth_transform::{SmoothTransformTo, smooth_transform},
    utils::get_edging_chunk,
};
use futures_lite::future;

pub struct AsyncChunkloaderPlugin;

pub const MAX_DATA_TASKS: usize = 64;
pub const MAX_MESH_TASKS: usize = 32;

impl Plugin for AsyncChunkloaderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AsyncChunkloader::default());
        // app.add_systems(Update, (start_data_tasks, start_mesh_tasks));
        app.add_systems(PostUpdate, (start_data_tasks, start_mesh_tasks));
        // app.add_systems(PostUpdate, (join_data, join_mesh));
        app.add_systems(Update, (start_modifications, smooth_transform));
        app.add_systems(
            // PostUpdate,
            Update,
            ((join_data, join_mesh), (unload_data, unload_mesh)).chain(),
        );
    }
}

/// holds all voxel world data
#[derive(Resource)]
pub struct AsyncChunkloader {
    pub world_data: HashMap<ChunkPosition, Arc<ChunkData>>,
    pub load_data_queue: Vec<ChunkPosition>,
    pub load_mesh_queue: Vec<ChunkPosition>,
    pub unload_data_queue: Vec<ChunkPosition>,
    pub unload_mesh_queue: Vec<ChunkPosition>,
    pub data_tasks: HashMap<ChunkPosition, Option<Task<ChunkData>>>,
    pub mesh_tasks: Vec<(ChunkPosition, Option<Task<Option<ChunkMesh>>>)>,
    pub chunk_entities: HashMap<ChunkPosition, Entity>,
    pub lod: Lod,
    pub chunk_modifications: HashMap<ChunkPosition, Vec<ChunkModification>>,
}

pub struct ChunkModification(pub RelativePosition, pub &'static BlockPrototype);

impl AsyncChunkloader {
    pub fn unload_all_meshes(&mut self, scanner: &Scanner, scanner_transform: &GlobalTransform) {
        // stop all any current proccessing
        self.load_mesh_queue.clear();
        // self.unload_mesh_queue.clear();
        self.mesh_tasks.clear();
        let translation = Position(scanner_transform.translation().as_ivec3());
        let scan_pos: ChunkPosition = translation.into();
        for offset in &scanner.mesh_sampling_offsets {
            let wpos = scan_pos + *offset;
            self.load_mesh_queue.push(wpos);
            // self.unload_mesh_queue.push(wpos);
        }
    }
}

impl Default for AsyncChunkloader {
    fn default() -> Self {
        assert!(
            Lod::default().size() == CHUNK_SIZE_I32,
            "Default LOD must exactly equal the chunk size."
        );

        Self {
            world_data: HashMap::default(),
            load_data_queue: Vec::new(),
            load_mesh_queue: Vec::new(),
            unload_data_queue: Vec::new(),
            unload_mesh_queue: Vec::new(),
            data_tasks: HashMap::default(),
            mesh_tasks: Vec::new(),
            chunk_entities: HashMap::default(),
            lod: Lod::default(),
            chunk_modifications: HashMap::default(),
        }
    }
}

/// begin data building tasks for chunks in range
#[allow(clippy::needless_pass_by_value)]
pub fn start_data_tasks(
    mut voxel_engine: ResMut<AsyncChunkloader>,
    block_prototypes: Res<BlockPrototypes>,
    scanners: Query<&GlobalTransform, With<Scanner>>,
) {
    let task_pool = AsyncComputeTaskPool::get();

    let AsyncChunkloader {
        load_data_queue,
        data_tasks,
        ..
    } = voxel_engine.as_mut();

    let scanner_g = scanners.single();

    let translation = Position(scanner_g.translation().as_ivec3());
    let scan_pos: ChunkPosition = translation.into();

    load_data_queue.sort_by(|a, b| {
        a.0.distance_squared(scan_pos.0)
            .cmp(&b.0.distance_squared(scan_pos.0))
    });

    let tasks_left = (MAX_DATA_TASKS as i32 - data_tasks.len() as i32)
        .min(load_data_queue.len() as i32)
        .max(0) as usize;
    for chunk_position in load_data_queue.drain(0..tasks_left) {
        let k = chunk_position;
        let prototypes = block_prototypes.clone();
        let task = task_pool.spawn(async move { ChunkData::generate(&prototypes, k) });
        data_tasks.insert(chunk_position, Some(task));
    }
}

/// destroy enqueued, chunk data
pub fn unload_data(mut voxel_engine: ResMut<AsyncChunkloader>) {
    let AsyncChunkloader {
        unload_data_queue,
        world_data,
        ..
    } = voxel_engine.as_mut();
    for chunk_pos in unload_data_queue.drain(..) {
        world_data.remove(&chunk_pos);
    }
}

/// destroy enqueued, chunk mesh entities
pub fn unload_mesh(mut commands: Commands, mut voxel_engine: ResMut<AsyncChunkloader>) {
    let AsyncChunkloader {
        unload_mesh_queue,
        chunk_entities,
        ..
    } = voxel_engine.as_mut();
    let mut retry = Vec::new();
    for chunk_pos in unload_mesh_queue.drain(..) {
        let Some(chunk_id) = chunk_entities.remove(&chunk_pos) else {
            continue;
        };
        if let Some(mut entity_commands) = commands.get_entity(chunk_id) {
            entity_commands.despawn();
        }
        // world_data.remove(&chunk_pos);
    }
    unload_mesh_queue.append(&mut retry);
}

/// begin mesh building tasks for chunks in range
#[allow(clippy::needless_pass_by_value)]
pub fn start_mesh_tasks(
    mut voxel_engine: ResMut<AsyncChunkloader>,
    scanners: Query<&GlobalTransform, With<Scanner>>,
) {
    let task_pool = AsyncComputeTaskPool::get();

    let AsyncChunkloader {
        load_mesh_queue,
        mesh_tasks,
        world_data,
        lod,
        ..
    } = voxel_engine.as_mut();

    let scanner_g = scanners.single();
    let scan_position: ChunkPosition = Position(scanner_g.translation().as_ivec3()).into();
    load_mesh_queue.sort_by(|a, b| {
        a.0.distance_squared(scan_position.0)
            .cmp(&b.0.distance_squared(scan_position.0))
    });
    let tasks_left = (MAX_MESH_TASKS as i32 - mesh_tasks.len() as i32)
        .min(load_mesh_queue.len() as i32)
        .max(0) as usize;
    for chunk_position in load_mesh_queue.drain(0..tasks_left) {
        let Some(chunks_refs) = ChunksRefs::try_new(&world_data, chunk_position) else {
            continue;
        };
        let llod = *lod;
        let task = task_pool
            .spawn(async move { greedy_mesher_optimized::build_chunk_mesh(&chunks_refs, llod) });

        mesh_tasks.push((chunk_position, Some(task)));
    }
}

pub fn start_modifications(mut voxel_engine: ResMut<AsyncChunkloader>) {
    let AsyncChunkloader {
        world_data,
        chunk_modifications,
        load_mesh_queue,
        ..
    } = voxel_engine.as_mut();
    for (pos, mods) in chunk_modifications.drain() {
        // say i want to load mesh now :)
        let Some(chunk_data) = world_data.get_mut(&pos) else {
            continue;
        };
        let new_chunk_data = Arc::make_mut(chunk_data);
        let mut adj_chunk_set = HashSet::new();
        for ChunkModification(local_pos, block_type) in mods {
            new_chunk_data.set_block(local_pos.into(), block_type);
            if let Some(edge_chunk) = get_edging_chunk(local_pos) {
                adj_chunk_set.insert(edge_chunk);
            }
        }
        for adj_chunk in adj_chunk_set {
            load_mesh_queue.push(pos + adj_chunk);
        }
        load_mesh_queue.push(pos);
    }
}

/// join the chunkdata threads
pub fn join_data(mut voxel_engine: ResMut<AsyncChunkloader>) {
    let AsyncChunkloader {
        world_data,
        data_tasks,
        ..
    } = voxel_engine.as_mut();
    for (chunk_position, task_option) in data_tasks.iter_mut() {
        let Some(mut task) = task_option.take() else {
            // should never happend, because we drop None values later
            warn!("someone modified task?");
            continue;
        };
        let Some(chunk_data) = block_on(future::poll_once(&mut task)) else {
            *task_option = Some(task);
            continue;
        };

        world_data.insert(*chunk_position, Arc::new(chunk_data));
    }
    data_tasks.retain(|_k, op| op.is_some());
}

#[derive(Component)]
pub struct WaitingToLoadMeshTag;

pub fn promote_dirty_meshes(
    mut commands: Commands,
    children: &Query<(Entity, &Mesh3d, &ChildOf), With<WaitingToLoadMeshTag>>,
    mut parents: Query<&mut Mesh3d, Without<WaitingToLoadMeshTag>>,
    asset_server: &Res<AssetServer>,
) {
    for (entity, handle, parent) in children.iter() {
        if let Some(state) = asset_server.get_load_state(&handle.0) {
            match state {
                LoadState::Loaded => {
                    let Ok(mut parent_handle) = parents.get_mut(parent.get()) else {
                        continue;
                    };
                    info!("updgraded!");
                    parent_handle.0 = handle.0.clone();
                    commands.entity(entity).despawn();
                }
                LoadState::Loading => {
                    info!("loading cool");
                }
                LoadState::NotLoaded => (),
                LoadState::Failed(error) => eprintln!("Could not load asset! Error: {error}"),
            }
        }
    }
}

/// join the multithreaded chunk mesh tasks, and construct a finalized chunk entity
#[allow(clippy::needless_pass_by_value)]
pub fn join_mesh(
    mut voxel_engine: ResMut<AsyncChunkloader>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    global_chunk_material: Res<GlobalChunkMaterial>,
    timer: Res<Time>,
) {
    let AsyncChunkloader {
        mesh_tasks,
        chunk_entities,
        ..
    } = voxel_engine.as_mut();
    for (chunk_position, task_option) in mesh_tasks.iter_mut() {
        let Some(mut task) = task_option.take() else {
            // should never happend, because we drop None values later
            warn!("someone modified task?");
            continue;
        };
        let Some(chunk_mesh_option) = block_on(future::poll_once(&mut task)) else {
            // failed polling, keep task alive
            *task_option = Some(task);
            continue;
        };

        let Some(mesh) = chunk_mesh_option else {
            continue;
        };
        let mut bevy_mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        bevy_mesh.insert_attribute(ATTRIBUTE_VOXEL, mesh.vertices.clone());
        bevy_mesh.insert_indices(Indices::U32(mesh.indices.clone()));
        let mesh_handle = meshes.add(bevy_mesh);

        if let Some(entity) = chunk_entities.get(chunk_position) {
            commands.entity(*entity).despawn();
        }

        // spawn chunk entity
        let chunk_entity = commands
            .spawn((
                Chunk,
                SmoothTransformTo::new(
                    &timer,
                    FloatingPosition::new(0., -CHUNK_INITIAL_Y_OFFSET, 0.),
                    CHUNK_FLOAT_UP_BLOCKS_PER_SECOND,
                ),
                Aabb::from_min_max(Vec3::ZERO, Vec3::splat(CHUNK_SIZE_F32)),
                Mesh3d(mesh_handle),
                MeshMaterial3d(global_chunk_material.0.clone()),
                Transform::from_translation(
                    (FloatingPosition::from(*chunk_position)
                        + FloatingPosition::new(0., CHUNK_INITIAL_Y_OFFSET, 0.))
                    .0,
                ),
            ))
            .id();
        chunk_entities.insert(*chunk_position, chunk_entity);
    }
    mesh_tasks.retain(|(_p, op)| op.is_some());
}

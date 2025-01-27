use std::sync::Arc;

use bevy::{
    math::{IVec3, ivec3},
    utils::HashMap,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::{
    chunk::ChunkData,
    quad::Direction,
    utils::{index_to_ivec3_bounds, vec3_to_index},
    voxel::BlockType,
};

// Pointers to chunk data, repersented as the middle one with all their neighbours in 3x3x3 cube.
#[derive(Clone)]
pub struct ChunksRefs {
    pub adjacent_chunks: [Arc<ChunkData>; 27],
}

impl ChunksRefs {
    /// construct a `ChunkRefs` at `middle_chunk` position
    /// # Panics
    /// if `ChunkData` doesn't exist in input `world_data`
    #[must_use]
    pub fn try_new(
        world_data: &HashMap<IVec3, Arc<ChunkData>>,
        middle_chunk: IVec3,
    ) -> Option<Self> {
        let adjacent_chunks: [Arc<ChunkData>; 27] = std::array::from_fn(|i| {
            let offset = index_to_ivec3_bounds(i as i32, 3) + IVec3::NEG_ONE;
            Arc::clone(
                world_data.get(&(middle_chunk + offset)).unwrap(),
            )
        });
        Some(Self { adjacent_chunks })
    }
    // returns if all the voxels are the same
    // this is an incredibly fast approximation (1 sample per chunk) all = voxels[0]
    // so may be inacurate, but the odds are incredibly low
    #[must_use]
    pub fn is_all_voxels_same(&self) -> bool {
        let first_block = self.adjacent_chunks[0].get_block_if_filled();
        let Some(block) = first_block else {
            return false;
        };
        for chunk in &self.adjacent_chunks[1..] {
            let option = chunk.get_block_if_filled();
            if let Some(v) = option {
                if block != v {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    /// Only used for test suite.
    #[must_use]
    pub fn make_dummy_chunk_refs(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        
        let pos = IVec3::new(
            rng.gen_range(-20..20),
            rng.gen_range(-5..5),
            rng.gen_range(-20..20),
        );

        let adjacent_chunks: [Arc<ChunkData>; 27] = std::array::from_fn(|i| {
            let offset = index_to_ivec3_bounds(i as i32, 3) + IVec3::NEG_ONE;
            Arc::clone(
                &Arc::new(ChunkData::generate(pos + offset)),
            )
        });

        Self { adjacent_chunks }
    }

    /// helper function to get block data that may exceed the bounds of the middle chunk
    /// input position is local pos to middle chunk
    #[must_use]
    pub fn get_block(&self, pos: IVec3) -> BlockType {
        let x = (pos.x + 32) as u32;
        let y = (pos.y + 32) as u32;
        let z = (pos.z + 32) as u32;
        let (x_chunk, x) = ((x / 32) as i32, (x % 32) as i32);
        let (y_chunk, y) = ((y / 32) as i32, (y % 32) as i32);
        let (z_chunk, z) = ((z / 32) as i32, (z % 32) as i32);

        let chunk_index = vec3_to_index(IVec3::new(x_chunk, y_chunk, z_chunk), 3);
        let chunk_data = &self.adjacent_chunks[chunk_index];
        let i = vec3_to_index(IVec3::new(x, y, z), 32);
        chunk_data.get_block(i)
    }

    /// helper function to get voxels
    /// panics if the local pos is outside the middle chunk
    #[must_use]
    pub fn get_block_no_neighbour(&self, pos: IVec3) -> BlockType {
        let chunk_data = &self.adjacent_chunks[13];
        let i = vec3_to_index(pos, 32);
        chunk_data.get_block(i)
    }

    /// helper function to sample adjacent(back,left,down) voxels
    #[must_use]
    pub fn get_adjacent_blocks(
        &self,
        pos: IVec3,
        // current back, left, down
    ) -> (BlockType, BlockType, BlockType, BlockType) {
        let current = self.get_block(pos);
        let back = self.get_block(pos + ivec3(0, 0, -1));
        let left = self.get_block(pos + ivec3(-1, 0, 0));
        let down = self.get_block(pos + ivec3(0, -1, 0));
        (current, back, left, down)
    }

    /// helper function to sample adjacent voxels, von neuman include all facing planes
    #[must_use]
    pub fn get_von_neumann(&self, pos: IVec3) -> Option<Vec<(Direction, BlockType)>> {
        Some(vec![
            (Direction::Back, self.get_block(pos + ivec3(0, 0, -1))),
            (Direction::Forward, self.get_block(pos + ivec3(0, 0, 1))),
            (Direction::Down, self.get_block(pos + ivec3(0, -1, 0))),
            (Direction::Up, self.get_block(pos + ivec3(0, 1, 0))),
            (Direction::Left, self.get_block(pos + ivec3(-1, 0, 0))),
            (Direction::Right, self.get_block(pos + ivec3(1, 0, 0))),
        ])
    }

    #[must_use]
    pub fn get_2(&self, pos: IVec3, offset: IVec3) -> (BlockType, BlockType) {
        let first = self.get_block(pos);
        let second = self.get_block(pos + offset);
        (first, second)
    }
}

use bevy::prelude::*;
use bracket_noise::prelude::*;

use crate::{
    utils::index_to_ivec3,
    voxel::BlockType,
};

#[derive(Clone)]
pub struct ChunkData {
    pub voxels: Vec<BlockType>,
}

impl ChunkData {
    #[inline]
    #[must_use]
    pub fn get_block(&self, index: usize) -> BlockType {
        if self.voxels.len() == 1 {
            self.voxels[0]
        } else {
            self.voxels[index]
        }
    }

    // returns the block type if all voxels are the same
    #[inline]
    #[must_use]
    pub fn get_block_if_filled(&self) -> Option<&BlockType> {
        if self.voxels.len() == 1 {
            Some(&self.voxels[0])
        } else {
            None
        }
    }

    /// shape our voxel data based on the `chunk_pos`
    #[must_use]
    pub fn generate(chunk_pos: IVec3) -> Self {
        // hardcoded extremity check
        if chunk_pos.y * 32 + 32 > 21 + 32 {
            return Self {
                voxels: vec![BlockType::Air],
            };
        }
        // hardcoded extremity check
        if chunk_pos.y * 32 < -21 - 32 {
            return Self {
                voxels: vec![BlockType::Grass],
            };
        }
        let mut voxels = vec![];
        let mut fast_noise = FastNoise::new();
        fast_noise.set_frequency(0.0254);
        for i in 0..32 * 32 * 32 {
            let voxel_pos = (chunk_pos * 32) + index_to_ivec3(i);
            let scale = 1.0;
            fast_noise.set_frequency(0.0254);
            let overhang = fast_noise.get_noise3d(
                voxel_pos.x as f32 * scale,
                voxel_pos.y as f32,
                voxel_pos.z as f32 * scale,
            ) * 55.0;
            fast_noise.set_frequency(0.002591);
            let noise_2 =
                fast_noise.get_noise(voxel_pos.x as f32 + overhang, voxel_pos.z as f32 * scale);
            let h = noise_2 * 30.0;
            let solid = h > voxel_pos.y as f32;

            let block_type = if !solid {
                BlockType::Air
            } else if (h - voxel_pos.y as f32) > 1.0 {
                BlockType::Dirt
            } else {
                BlockType::Grass
            };
            voxels.push(block_type);
        }

        Self { voxels }
    }
}

use bevy_utils::HashMap;
use glam::{IVec3, Quat, Vec3Swizzles, Vec3, IVec2};

use crate::{
    game::states::game::{self, GameState},
    resources::{game_state, Resources},
};

use super::chunk::{Chunk, CHUNK_SIZE};

pub type ECS = hecs::World;
pub type ChunkIndex = u32;

pub const WORLD_HEIGHT: usize = 256;
pub const WORLD_HEIGHT_CHUNKS: usize = WORLD_HEIGHT / CHUNK_SIZE;

pub struct ChunkRenderData {}

pub struct Chunks {
    corner_chunk_pos: IVec2,
    chunks: Box<[Option<Box<Chunk>>]>,
    render_data: Box<[Option<ChunkRenderData>]>,
    render_distance: u32,
}

impl Chunks {
    pub fn new(render_distance: u32, player_chunk_pos: IVec3) -> Self {
        let n = 2 * render_distance as usize;

        let chunks = std::iter::repeat_with(|| None::<Box<Chunk>>)
            .take(n * n * WORLD_HEIGHT_CHUNKS)
            .collect::<Box<[_]>>();

        let render_data = std::iter::repeat_with(|| None::<ChunkRenderData>)
            .take(n * n * WORLD_HEIGHT_CHUNKS)
            .collect::<Box<[_]>>();

        Self {
            corner_chunk_pos: player_chunk_pos.xz() - render_distance as i32,
            chunks,
            render_data,
            render_distance
        }
    }

    pub fn get_at(&self, pos: IVec3) -> Option<&Chunk> {
        self.chunks[self.pos_to_idx(pos) as usize].as_deref()
    }

    pub fn get_at_mut(&mut self, pos: IVec3) -> Option<&mut Chunk> {
        self.chunks[self.pos_to_idx(pos) as usize].as_deref_mut()
    }

    pub fn remove(&mut self, index: ChunkIndex) -> Option<Box<Chunk>> {
        let chunk = std::mem::take(&mut self.chunks[index as usize]);
        std::mem::take(&mut self.render_data[index as usize]);
        chunk
    }

    fn pos_to_idx(&self, pos: IVec3) -> ChunkIndex {
        let grid_xz = (pos.xz() - self.corner_chunk_pos).as_uvec2() & 127;
        ((pos.y as u32 * 128 * 128) | (grid_xz.x * 128) | grid_xz.y) as ChunkIndex
    }

    pub fn on_player_exited_chunk(&mut self, new_chunk_pos: IVec3) {
        let new_corner_pos = new_chunk_pos.xz() - self.render_distance as i32;
        let change = new_corner_pos - self.corner_chunk_pos;


    }
}

impl Chunks {
    pub fn tick(&mut self, res: &mut Resources) -> anyhow::Result<()> {
        Ok(())
    }
}

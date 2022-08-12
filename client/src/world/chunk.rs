use glam::IVec3;

use super::block::Block;

pub const CHUNK_SIZE_LOG2: usize = 4;
pub const CHUNK_SIZE: usize = 1 << CHUNK_SIZE_LOG2;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

pub type WorldBlockPos = IVec3;

pub trait WorldBlockPosExt {
    fn to_block_index(self) -> usize;

    fn to_local(self) -> ChunkBlockPos;

    fn to_chunk_pos(self) -> IVec3;
}

impl WorldBlockPosExt for WorldBlockPos {
    fn to_block_index(self) -> usize {
        const XZ_BITS: usize = 24;

        ((self.y as u32 as usize) << (2 * XZ_BITS))
            | ((self.z as u32 as usize) << XZ_BITS)
            | (self.x as u32 as usize)
    }

    fn to_local(self) -> ChunkBlockPos {
        ChunkBlockPos::new(self.x as u8, self.y as u8, self.z as u8)
    }

    fn to_chunk_pos(self) -> IVec3 {
        self >> CHUNK_SIZE_LOG2 as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkBlockPos {
    pub x: u8,
    pub y: u8,
    pub z: u8,
    pub extra: u8,
}

impl ChunkBlockPos {
    pub const COORD_MASK: u8 = CHUNK_SIZE as u8 - 1;

    pub const fn new(x: u8, y: u8, z: u8) -> Self {
        Self {
            x: x & Self::COORD_MASK,
            y: y & Self::COORD_MASK,
            z: z & Self::COORD_MASK,
            extra: 0,
        }
    }

    pub const fn to_block_index(self) -> usize {
        self.z as usize * CHUNK_SIZE * CHUNK_SIZE + self.x as usize * CHUNK_SIZE + self.y as usize
    }
}

impl<T> From<(T, T, T)> for ChunkBlockPos
where
    T: Into<u8>,
{
    fn from((x, y, z): (T, T, T)) -> Self {
        Self::new(x.into(), y.into(), z.into())
    }
}

impl From<WorldBlockPos> for ChunkBlockPos {
    fn from(pos: WorldBlockPos) -> Self {
        pos.to_local()
    }
}

pub struct Chunk {
    blocks: [Block; CHUNK_VOLUME],
    pub dirty: bool,
    // Id of the 2³ chunk group this chunk belongs to
    pub group_id: thunderdome::Index,

    pub neighbor_indices: [u32; 6],
}

impl Chunk {
    pub fn new(group_id: thunderdome::Index, neighbor_indices: [u32; 6]) -> Box<Self> {
        let mut boxed = unsafe {
            let layout = std::alloc::Layout::new::<Chunk>();
            let mem = std::alloc::alloc_zeroed(layout);
            if mem.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            Box::from_raw(mem.cast::<Chunk>())
        };
        boxed.group_id = group_id; // TODO probably UB to do this here instead of before mem.cast()
        boxed.neighbor_indices = neighbor_indices;

        // For some reason, the compiler does not optimize the memset away,
        // even though Block is made of zero bytes and the memory is already
        // zero-initialized.
        const _: () = assert!(
            Block::AIR.raw() == 0,
            "Chunk::new(): air block no longer zero, needs memset"
        );
        // boxed.blocks.fill(Block::new(BlockId::AIR));

        boxed
    }

    pub fn blocks(&self) -> &[Block; CHUNK_VOLUME] {
        &self.blocks
    }

    pub fn fill(&mut self, block: Block) {
        self.blocks.fill(block);
    }
}

impl std::ops::Index<usize> for Chunk {
    type Output = Block;

    fn index(&self, index: usize) -> &Self::Output {
        &self.blocks[index]
    }
}

impl<T> std::ops::Index<T> for Chunk
where
    T: Into<ChunkBlockPos>,
{
    type Output = Block;

    fn index(&self, index: T) -> &Self::Output {
        let pos: ChunkBlockPos = index.into();
        &self[pos.to_block_index()]
    }
}

impl std::ops::IndexMut<usize> for Chunk {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.blocks[index]
    }
}

impl<T> std::ops::IndexMut<T> for Chunk
where
    T: Into<ChunkBlockPos>,
{
    fn index_mut(&mut self, index: T) -> &mut Self::Output {
        let pos: ChunkBlockPos = index.into();
        &mut self[pos.to_block_index()]
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChunkFace {
    NX,
    NY,
    NZ,
    PX,
    PY,
    PZ,
}

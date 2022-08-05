
pub struct BlockData(u16);

impl BlockData {
    const COMPLEX_MASK : u16 = 1 << 15; // MSB
    
    // Complex blocks use first 15 bits as an index to a separate table of blocks, because
    // one complex block consists of 8 blocks
    pub fn is_complex(self) -> bool {
        (self.0 & Self::COMPLEX_MASK) != 0
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BlockId(u16);

impl BlockId {
    pub const AIR: BlockId = BlockId(0);
    pub const STONE : BlockId = BlockId(1);
}

impl BlockId {
    // Either full or partial transparency
    pub fn is_transparent(self) -> bool {
        self == Self::AIR
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Block(u16);

impl Block {
    pub const fn new(id: BlockId) -> Self {
        Self(id.0)
    }

    pub const fn raw(self) -> u16 {
        self.0
    }

    pub const fn data(self) -> BlockData {
        BlockData(self.0 >> 10)
    }

    pub const fn id(self) -> BlockId {
        BlockId(self.0 & ((1 << 10) - 1))
    }
}

impl Block {
    pub const AIR : Block = Block::new(BlockId::AIR);
    pub const STONE : Block = Block::new(BlockId::STONE);
}

impl From<Block> for BlockId {
    fn from(block: Block) -> Self {
        block.id()
    }
}
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

    pub const fn id(self) -> BlockId {
        BlockId(self.0 & 0x3FF)
    }
}

impl From<Block> for BlockId {
    fn from(block: Block) -> Self {
        block.id()
    }
}
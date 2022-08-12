pub struct ChunkGenerator {
    world_seed: u64,
}

impl ChunkGenerator {
    pub fn new(world_seed: u64) -> Self {
        Self { world_seed }
    }
}

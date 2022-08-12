use thunderdome::Arena;

// Render data for a 2³ group of chunks, i.e for a 32³ block volume
struct ChunkGroupRenderData {}

pub struct ChunkRenderer {
    chunk_render_data: Arena<ChunkGroupRenderData>,
}

impl ChunkRenderer {
    pub fn new() -> Self {
        Self {
            chunk_render_data: Arena::new(),
        }
    }
}

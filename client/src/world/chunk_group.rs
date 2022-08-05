use thunderdome::Arena;




pub struct ChunkGroupData {
}

pub struct ChunkGroups {
    groups: Arena<ChunkGroupData>, // *The* arena responsible for index generation
}

impl ChunkGroups {
    pub fn new() -> Self {
        Self {
            groups: Arena::new()
        }
    }
}
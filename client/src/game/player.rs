use std::sync::{Arc, atomic::AtomicU32};

use glam::Vec3;


pub struct ThePlayer {
    pub pos: Vec3,
}

impl ThePlayer {
    pub fn new(pos: Vec3) -> Self {
        Self { pos }
    }
}


/* pub struct Shared {
    buf_idx: AtomicU32,
    buffers: [Vec<u8>; 3]
}

pub struct ByteChannel {
    shared: Arc<Shared>,
    buf_idx: u32,
    buf: Vec<u8>
}

impl ByteChannel {
    pub fn new() -> (ByteChannel, ByteChannel) {
        let shared = Arc::new(Shared {
            buf_idx: AtomicU32::new(0),
            buffers: [Vec::default(), Vec::default(), Vec::default()]
        });

        (
            ByteChannel {
                shared: shared.clone(),
                buf_idx: 0,
                buf: Vec::default()
            },
            ByteChannel {
                shared: shared,
                buf_idx: 1,
                buf: Vec::default()
            }
        )
    }
} */
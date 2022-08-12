use glam::Vec3;

pub struct ThePlayer {
    pub pos: Vec3,
    pub vel: Vec3,
}

impl ThePlayer {
    pub fn new(pos: Vec3) -> Self {
        Self {
            pos,
            vel: Vec3::ZERO,
        }
    }
}

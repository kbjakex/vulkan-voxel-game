use glam::{Vec2, Vec3};

#[derive(Clone, Copy)]
pub struct Position(pub Vec3);

#[derive(Clone, Copy)]
pub struct OldPosition(pub Vec3);

// A position that has been received, but is not yet being rendered
#[derive(Clone, Copy)]
pub struct PendingPosition(pub Vec3);

#[derive(Clone, Copy)]
pub struct HeadRotation(pub Vec2);

#[derive(Clone, Copy)]
pub struct OldHeadRotation(pub Vec2);

#[derive(Clone, Copy)]
pub struct PendingHeadRotation(pub Vec2);

#[derive(Clone, Copy)]
pub struct Velocity(pub Vec3);

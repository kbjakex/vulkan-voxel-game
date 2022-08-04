
// At-a-glance view of all components in the game.
// Should preferably be imported from here for consistency and convenience,
// although in practice there is no difference.

use glam::Vec3;

#[derive(Clone, Copy)]
pub struct Position {
    pub xyz: Vec3,
}

pub mod net {
    pub type NetworkId = shared::protocol::NetworkId;
    pub type PlayerConnection = crate::net::PlayerConnection;
}

// At-a-glance view of all resources in the game.
// Should preferably be imported from here for consistency and convenience,
// although in practice there is no difference.

use hecs::World;

use crate::net::Network;

pub struct Resources {
    pub net: Network,
    pub main_world: World,
    pub time: Time
}

pub struct Time {
    pub at_launch: std::time::Instant, // never updated, measured just before game loop
    pub now: std::time::Instant,       // updated at the very start of each frame
    pub ms_u32: u32,
    pub secs_f32: f32,
}

// At-a-glance view of all components in the game.
// Should preferably be imported from here for consistency and convenience,
// although in practice there is no difference.

use flexstr::SharedStr;
use glam::{Vec3, Vec2};
use hecs::{Entity, World};

pub type YawPitch = Vec2;

pub trait YawPitchExt {
    fn as_yaw_pitch_to_dir(self) -> Vec3;
}

impl YawPitchExt for YawPitch {
    fn as_yaw_pitch_to_dir(self) -> Vec3 {
        let (yaw_sin, yaw_cos) = self.x.sin_cos(); 
        let (pitch_sin, pitch_cos) = self.y.sin_cos(); 
        Vec3 {
            x: yaw_cos * pitch_cos,
            y: pitch_sin,
            z: yaw_sin * pitch_cos,
        }
    }
}


#[derive(Clone, Copy)]
pub struct Position(pub Vec3);

#[derive(Clone, Copy)]
pub struct OldPosition(pub Vec3);

#[derive(Clone, Copy)]
pub struct Facing(pub Vec3);

#[derive(Clone, Copy)]
pub struct HeadYawPitch {
    pub value: YawPitch,
    pub delta: YawPitch,
}

pub struct Username(pub SharedStr);

// A server-internal player index. Kept as close to zero as possible
// so that data structures don't need to allocate much unnecessary space.
#[derive(Clone, Copy)]
pub struct PlayerId(u8);

impl PlayerId {
    pub const fn from_raw(raw: u8) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u8 {
        self.0
    }
}

pub type NetworkId = shared::protocol::NetworkId;


pub struct PlayerBundle {
    pub nid: NetworkId,
    pub player_id: PlayerId,
    pub username: SharedStr,
    pub position: Vec3,
    pub head_rotation: YawPitch,
}

pub fn spawn_player(ecs: &mut World, bundle: PlayerBundle) -> Entity {
    ecs.spawn((
        bundle.nid,
        bundle.player_id,
        Username(bundle.username),
        Position(bundle.position),
        OldPosition(bundle.position),
        Facing(bundle.head_rotation.as_yaw_pitch_to_dir()),
        HeadYawPitch {
            value: bundle.head_rotation,
            delta: YawPitch::ZERO,
        }
    ))
}

use std::{time::Instant, net::SocketAddr};

use crate::{resources::{Resources, Time}, net, components::{Position, OldPosition, HeadYawPitch}};

use anyhow::Result;
use glam::Vec2;
use hecs::World;
use shared::protocol;

pub fn tick(res: &mut Resources) -> anyhow::Result<()> {
    let now = Instant::now();
    let time_res = &mut res.time;
    time_res.now = now;
    time_res.secs_f32 = (now - time_res.at_launch).as_secs_f32();
    time_res.ms_u32 = (now - time_res.at_launch).as_millis() as u32;

    net::tick(res)?;

    // TODO: This could probably be done only just before an entity moves, assuming
    // entity moves is handled in few places.
    for (_, (&Position(new_pos), OldPosition(old_pos), head_rot)) 
        in res.main_world.query_mut::<(&Position, &mut OldPosition, &mut HeadYawPitch)>() {
        
        head_rot.value -= head_rot.delta;
        head_rot.value += protocol::round_angles(head_rot.delta);
        head_rot.delta = Vec2::ZERO;

        *old_pos += protocol::round_velocity(new_pos - *old_pos);
    }


    Ok(())
}

pub fn shutdown(res: Resources) {
    
}

pub fn init(address: SocketAddr) -> Result<Resources> {
    let now = Instant::now();

    Ok(Resources {
        net: crate::net::init(address)?,
        main_world: World::new(),
        time: Time {
            at_launch: now,
            now,
            ms_u32: 0,
            secs_f32: 0.0,
        },
        current_tick: 0,
    })
}

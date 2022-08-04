use std::time::Instant;

use crate::{resources::{Resources, Time}, net};

use anyhow::Result;
use hecs::World;

pub fn tick(res: &mut Resources) {
    let now = Instant::now();
    let time_res = &mut res.time;
    time_res.now = now;
    time_res.secs_f32 = (now - time_res.at_launch).as_secs_f32();
    time_res.ms_u32 = (now - time_res.at_launch).as_millis() as u32;

    net::tick(res);
}

pub fn shutdown(res: Resources) {
    
}

pub fn init() -> Result<Resources> {
    let now = Instant::now();

    Ok(Resources {
        net: crate::net::init()?,
        main_world: World::new(),
        time: Time {
            at_launch: now,
            now,
            ms_u32: 0,
            secs_f32: 0.0,
        }
    })
}

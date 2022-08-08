use glam::{Vec3, DVec3};
use shared::TICKS_PER_SECOND;
use smallvec::SmallVec;

use crate::components::Position;

pub struct Integrator {
    origin: DVec3,
    prev_vel: DVec3,
    vel_accum: DVec3,
    time_accum: f64,
    prev_dt: f64,
}

impl Integrator {
    pub fn new(origin: DVec3) -> Self {
        Self {
            origin,
            prev_vel: DVec3::ZERO,
            vel_accum: DVec3::ZERO,
            time_accum: 0.0,
            prev_dt: 0.0,
        }
    }

    // `vel` should be premultiplied by dt
    pub fn step(&mut self, vel: DVec3, dt_secs: f64, frame_velocities_out: &mut SmallVec<[Vec3; 4]>) -> Position {        
        const NW_TICK : f64 = 1.0 / TICKS_PER_SECOND as f64;

        self.time_accum += self.prev_dt;
        while self.time_accum >= NW_TICK {
            let carry_vel = self.prev_vel * (self.time_accum - NW_TICK) / self.prev_dt;

            let total_v = self.vel_accum - carry_vel;
            frame_velocities_out.push(total_v.as_vec3());

            self.time_accum -= NW_TICK;
            self.vel_accum = carry_vel;
            self.origin += total_v;
        }

        self.vel_accum += vel;

        self.prev_vel = vel;
        self.prev_dt = dt_secs;
        
        Position((self.origin + self.vel_accum).as_vec3())
    }
}

pub struct InputRecorder {
    pub integrator: Integrator
}

impl InputRecorder {
    pub fn new(position: Vec3) -> Self {
        Self {
            integrator: Integrator::new(position.as_dvec3())
        }
    }
}

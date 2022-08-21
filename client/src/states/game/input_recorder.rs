use std::f64::consts::PI;

use glam::{DVec2, DVec3, Vec2, Vec3};
use shared::{
    protocol::{wrap_angles, self},
    TICKS_PER_SECOND,
};

use crate::components::Position;

#[derive(Clone, Copy)]
pub struct YawPitch(pub f32, pub f32);

pub struct Integrator {
    vel_origin: Vec3,
    prev_vel: DVec3,
    vel_accum: DVec3,

    angle_origin: Vec2,
    prev_angle: DVec2,
    angle_accum: DVec2,

    time_accum: f64,
    prev_dt: f64,
}

impl Integrator {
    pub fn new(origin: Vec3) -> Self {
        Self {
            vel_origin: origin,
            prev_vel: DVec3::ZERO,
            vel_accum: DVec3::ZERO,
            angle_origin: Vec2::ZERO,
            angle_accum: DVec2::ZERO,
            prev_angle: DVec2::ZERO,
            time_accum: 0.0,
            prev_dt: 0.0,
        }
    }

    // `vel` should be premultiplied by dt. Angles are never multiplied by dt.
    pub fn step(
        &mut self,
        vel: DVec3,
        yaw_pitch: DVec2,
        dt_secs: f64,
        mut input_id: u16,
        snapshots_out: &mut Vec<InputSnapshot>,
    ) -> (Position, YawPitch) {
        const NW_TICK: f64 = 1.0 / TICKS_PER_SECOND as f64;

        self.time_accum += self.prev_dt;
        while self.time_accum >= NW_TICK {
            let k = (self.time_accum - NW_TICK) / self.prev_dt;
            let carry_v = self.prev_vel * k;
            let carry_a = self.prev_angle * k;

            let total_v = protocol::round_velocity((self.vel_accum - carry_v).as_vec3());
            let total_a = wrap_angles(protocol::round_angles((self.angle_accum - carry_a).as_vec2()));

            self.time_accum -= NW_TICK;
            self.vel_accum = carry_v;
            self.angle_accum = carry_a;
            self.vel_origin += total_v;
            self.angle_origin = wrap_angles(self.angle_origin + total_a);

            snapshots_out.push(InputSnapshot {
                tag: input_id, 
                delta_position: total_v,
                delta_rotation: total_a,
                client_pos: self.vel_origin 
            });
            input_id += 1;
        }

        let mut yaw_pitch = yaw_pitch; // mutable
        const EPS: f64 = 0.001;
        if self.angle_origin.y as f64 + self.angle_accum.y + yaw_pitch.y >= PI / 2.0 - EPS {
            yaw_pitch.y = PI / 2.0 - EPS - self.angle_origin.y as f64 - self.angle_accum.y;
        }
        if self.angle_origin.y as f64 + self.angle_accum.y + yaw_pitch.y <= -PI / 2.0 + EPS {
            yaw_pitch.y = -PI / 2.0 + EPS - self.angle_origin.y as f64 - self.angle_accum.y;
        }

        self.vel_accum += vel;
        self.angle_accum += yaw_pitch;
        self.prev_vel = vel;
        self.prev_angle = yaw_pitch;
        self.prev_dt = dt_secs;

        let pos = self.vel_origin + protocol::round_velocity(self.vel_accum.as_vec3());
        let angles = self.angle_origin + protocol::round_angles(self.angle_accum.as_vec2());
        (Position(pos), YawPitch(angles.x, angles.y))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InputSnapshot {
    pub tag: u16,
    pub delta_position: Vec3, // also goes by 'velocity'
    pub delta_rotation: Vec2,

    pub client_pos: Vec3,
}

pub struct InputRecorder {
    integrator: Integrator,
    input_id: u16,
    input_history: Vec<InputSnapshot>
}

impl InputRecorder {
    pub fn new(position: Vec3) -> Self {
        Self {
            integrator: Integrator::new(position),
            input_id: 0,
            input_history: Vec::new(),
        }
    }

    pub fn predictions(&self) -> &[InputSnapshot] {
        &self.input_history
    }

    // returns true if prediction had likely failed (not exact and shouldn't be treated as exact)
    pub fn process_server_authoritative_state(
        &mut self,
        tag: u16,
        position: Vec3,
        head_rotation: Vec2,
    ) -> bool {
        let tag = tag.wrapping_add(1);

        let oldest_id = self.input_id.wrapping_sub(self.input_history.len() as u16);
        let to_remove = tag.wrapping_sub(oldest_id);
        if to_remove == 0 || to_remove > self.input_history.len() as u16 {
            return false;
        }

        self.input_history.drain(..to_remove as usize);
   
        //print!("{} vs {} vs {} ({}); ", inp.tag, tag, self.input_id, (tag as i32) - self.input_id as i32);
        /*assert_eq!(inp.tag, tag);*/
    
        /* println!("Server pos: {:.8} {:.8} {:.8}, predicted {:.8} {:.8} {:.8}", 
            position.x, position.y, position.z, 
            self.integrator.vel_origin.x, self.integrator.vel_origin.y,self.integrator.vel_origin.z,
        ); */

        let (new_pos, new_rotation) = self.input_history.iter()
            .fold((position, head_rotation), |accum, rhs| {
                (accum.0 + rhs.delta_position, accum.1 + rhs.delta_rotation)
            });

        //println!("Pos difference: {}, rot difference: {}", self.integrator.vel_origin.distance(new_pos), self.integrator.angle_origin.distance(new_rotation));

        let failed = !new_pos.abs_diff_eq(self.integrator.vel_origin, 0.005);

        self.integrator.angle_origin = new_rotation;
        self.integrator.vel_origin = new_pos;

        failed
    }

    pub fn record(
        &mut self, 
        velocity: Vec3, 
        head_rotation: Vec2, 
        dt_secs: f32
    ) -> (Position, YawPitch) {
        let old_len = self.input_history.len();

        let new_state = self.integrator.step(
            velocity.as_dvec3() * dt_secs as f64, 
            head_rotation.as_dvec2(), 
            dt_secs as f64, 
            self.input_id,
            &mut self.input_history
        );

        self.input_id = self.input_id.wrapping_add((self.input_history.len() - old_len) as u16);

        if old_len != self.input_history.len() && self.predictions().last().unwrap().delta_position != Vec3::ZERO {
            //let o = self.integrator.vel_origin;
            //println!("Pos @ {}: {:.8}, {:.8}, {:.8}", self.input_id, o.x, o.y, o.z);
        }

        new_state
    }
}

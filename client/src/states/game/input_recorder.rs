use std::f64::consts::PI;

use glam::{vec2, vec3, DVec2, DVec3, Vec2, Vec3};
use shared::{
    protocol::{decode_angle_rad, decode_velocity, encode_angle_rad, encode_velocity, wrap_angle},
    TICKS_PER_SECOND,
};
use smallvec::SmallVec;

use crate::components::{Position, Velocity};

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
        frame_velocities_out: &mut SmallVec<[(Velocity, YawPitch); 4]>,
    ) -> (Position, YawPitch) {
        const NW_TICK: f64 = 1.0 / TICKS_PER_SECOND as f64;

        self.time_accum += self.prev_dt;
        while self.time_accum >= NW_TICK {
            let k = (self.time_accum - NW_TICK) / self.prev_dt;
            let carry_v = self.prev_vel * k;
            let carry_a = self.prev_angle * k;

            let total_v = Self::round_velocity(self.vel_accum - carry_v);
            let total_a = Self::round_angles(self.angle_accum - carry_a);

            self.time_accum -= NW_TICK;
            self.vel_accum = carry_v;
            self.angle_accum = carry_a;
            self.vel_origin += total_v;
            self.angle_origin += total_a;

            frame_velocities_out.push((
                Velocity(total_v),
                YawPitch(total_a.x as f32, total_a.y as f32),
            ));
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

        let pos = self.vel_origin + Self::round_velocity(self.vel_accum);
        let angles = self.angle_origin + Self::round_angles(self.angle_accum);
        (Position(pos), YawPitch(angles.x, angles.y))
    }

    fn round_velocity(vel: DVec3) -> Vec3 {
        let vel = vel.as_vec3();
        // Simulates the network compression and decompression
        // If max velocity per second is 128 blocks, then per network tick at 32 Hz the max is
        // 4 because a network tick is 1/32 of a second. At 1/512 block precision, a -4..4 range
        // of values requires 4096 values per axis, or 12 bits. Round that to 16 bits for now..
        let x = decode_velocity(encode_velocity(vel.x));
        let y = decode_velocity(encode_velocity(vel.y));
        let z = decode_velocity(encode_velocity(vel.z));

        //let res = Vec3::new(x, y, z);
        //println!("Length: {:.8} -> {:.8} (* {:.8})", vel.length(), res.length(), res.length()/vel.length());
        vec3(x, y, z)
    }

    /* fn round_velocity(vel: DVec3) -> Vec3 {
        let vel = vel.as_vec3();
        // Simulates the network compression and decompression
        let x = ((vel.x * 500.0 + 128.0).round() as i32).clamp(0, 255) as u8;
        let y = ((vel.y * 500.0 + 128.0).round() as i32).clamp(0, 255) as u8;
        let z = ((vel.z * 500.0 + 128.0).round() as i32).clamp(0, 255) as u8;

        let x = (x as i32 - 128) as f32 / 500.0;
        let y = (y as i32 - 128) as f32 / 500.0;
        let z = (z as i32 - 128) as f32 / 500.0;

        let res = Vec3::new(x, y, z);
        //println!("Length: {:.8} -> {:.8} (* {:.8})", vel.length(), res.length(), res.length()/vel.length());
        res
    } */

    fn round_angles(a: DVec2) -> Vec2 {
        let yaw = decode_angle_rad(encode_angle_rad(wrap_angle(a.x as f32)));
        let pitch = decode_angle_rad(encode_angle_rad(wrap_angle(a.y as f32)));
        vec2(yaw, pitch)
    }
}

pub struct InputRecorder {
    pub integrator: Integrator,
}

impl InputRecorder {
    pub fn new(position: Vec3) -> Self {
        Self {
            integrator: Integrator::new(position),
        }
    }
}

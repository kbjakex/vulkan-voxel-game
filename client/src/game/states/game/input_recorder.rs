use glam::{Vec3, DVec3};

use crate::{
    camera::Camera,
    input::{keyboard, Key, Keyboard},
    resources::Resources,
};

pub struct PositionIntegrator {
    origin: Vec3,
    accumulator: DVec3,
    last_velocity: DVec3,
    start_time_secs: f64,
    last_update_secs: f64,

    time_last_frame: f64,
    time_two_frames_ago: f64,
    time_three_frames_ago: f64,
    raw_velocity_last_frame: DVec3,

    pub pos: Vec3
}

impl PositionIntegrator {
    pub fn new(origin: Vec3, time: f32) -> Self {
        Self {
            origin: origin,
            accumulator: DVec3::ZERO,
            last_velocity: DVec3::ZERO,
            start_time_secs: time as _,
            last_update_secs: time as _,
            pos: origin,

            time_last_frame: 0.0,
            time_two_frames_ago: 0.0,
            time_three_frames_ago: 0.0,
            raw_velocity_last_frame: DVec3::ZERO, // not scaled by dt
        }
    }

    pub fn update(&mut self, keyboard: Option<&mut Keyboard>, camera: &Camera, speed: f32, time_secs: f32) -> Vec3 {
        self.last_velocity = DVec3::ZERO;
        self.raw_velocity_last_frame = DVec3::ZERO;

        let old_mag = self.accumulator.length();

        if let Some(keyboard) = keyboard {
            let right = keyboard.get_axis(Key::D, Key::A);
            let up = keyboard.get_axis(Key::Space, Key::LShift);
            let fwd = keyboard.get_axis(Key::W, Key::S);

            if right != 0 || up != 0 || fwd != 0 {
                let (ys, yc) = camera.yaw().sin_cos();
                let fwd_dir = DVec3::new(yc as f64, 0.0, ys as f64);
                let up_dir = DVec3::Y;
                let right_dir = fwd_dir.cross(up_dir);

                let velocity =
                    (right as f64) * right_dir + (fwd as f64) * fwd_dir + (up as f64) * up_dir;
                    
                self.raw_velocity_last_frame = velocity.normalize() * speed as f64;

                //println!("dt: {}", time_secs as f64 - self.last_update_secs);
                let speed = speed as f64 * (time_secs as f64 - self.last_update_secs);
                self.last_velocity = velocity.normalize() * speed;
                self.accumulator += self.last_velocity;

                self.pos = self.origin + Self::round_velocity(self.accumulator);
            }
        }

        let dt = time_secs as f64 - self.last_update_secs;

        self.time_three_frames_ago = self.time_two_frames_ago;
        self.time_two_frames_ago = self.time_last_frame;
        self.time_last_frame = time_secs as f64;

        //println!("t: {time_secs:.8} @ update(), dt = {:.8}, mag: {:.8} -> {:.8}", dt, old_mag, self.accumulator.length());

        self.last_update_secs = time_secs as f64;
        self.pos
    }

    fn round_velocity(vel: DVec3) -> Vec3 {
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
    }

    pub fn end_network_tick(&mut self, time_secs: f32, network_tick_time_secs: f32) -> Vec3 {
        let nw_time = network_tick_time_secs as f64;
        let last_dt = self.time_last_frame - self.time_two_frames_ago;
        let t = (nw_time - self.time_two_frames_ago - (self.time_two_frames_ago - self.time_three_frames_ago)) / last_dt;

        let final_accum = Self::round_velocity(self.accumulator - self.last_velocity + t * self.last_velocity);
        let new_accum = (time_secs as f64 - nw_time) / last_dt * self.last_velocity;

        self.origin = self.origin + final_accum;
        self.accumulator = new_accum;

        self.pos = self.origin;

        println!("t: {time_secs:.8}, Mag: {:.8}, overflow: {:.8}", final_accum.length(), new_accum.length());

        final_accum
    }
}

pub struct InputRecorder {
    pub integrator: PositionIntegrator
}

impl InputRecorder {
    pub fn new(position: Vec3, time_secs: f32) -> Self {
        Self {
            integrator: PositionIntegrator::new(position, time_secs)
        }
    }
}

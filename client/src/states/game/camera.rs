use std::f32::consts::PI;

use glam::{Mat4, Vec2, Vec3};

pub struct Camera {
    projection: Mat4,
    view: Mat4,
    proj_view: Mat4,

    facing: Vec3,
    right: Vec3,
    yaw: f32,
    pitch: f32,

    pos: Vec3,

    fov: f32,
}

impl Camera {
    pub fn new(pos: Vec3, win_size: Vec2, fov_rad: f32) -> Self {
        let facing = euler_to_vec(0.0, 0.0);
        let projection = Self::create_projection_matrix(fov_rad, win_size);
        let view = Mat4::look_at_rh(pos, pos + facing, Vec3::Y);
        Camera {
            projection,
            view,
            proj_view: projection * view,
            facing,
            right: compute_right(facing),
            yaw: 0.0,
            pitch: 0.0,
            pos,
            fov: fov_rad,
        }
    }

    pub fn update(&mut self) {
        self.view = Mat4::look_at_rh(self.pos, self.pos + self.facing, Vec3::Y);
        self.proj_view = self.projection * self.view;
    }

    pub fn rotate(&mut self, yaw_delta_rad: f32, pitch_delta_rad: f32) {
        self.yaw += yaw_delta_rad;
        self.pitch = (self.pitch - pitch_delta_rad).clamp(-PI / 2.0 + 0.001, PI / 2.0 - 0.001);

        self.set_rotation(self.yaw, self.pitch);
    }

    pub fn set_rotation(&mut self, yaw_rad: f32, pitch_rad: f32) {
        self.yaw = yaw_rad % std::f32::consts::TAU;
        if self.yaw < 0.0 {
            self.yaw += std::f32::consts::TAU;
        }
        self.pitch = pitch_rad;
        self.facing = euler_to_vec(self.yaw, self.pitch);
        self.right = compute_right(self.facing);
    }

    pub fn set_fov(&mut self, fov_rad: f32, win_size: Vec2) {
        self.fov = fov_rad;
        self.on_window_resize(win_size);
    }

    pub fn on_window_resize(&mut self, new_size: Vec2) {
        self.projection = Self::create_projection_matrix(self.fov, new_size);
    }

    pub fn move_by(&mut self, velocity: Vec3) {
        self.pos += velocity;
    }

    pub fn move_to(&mut self, pos: Vec3) {
        self.pos = pos;
    }

    pub fn pos(&self) -> Vec3 {
        self.pos
    }

    pub fn facing(&self) -> Vec3 {
        self.facing
    }

    pub fn right(&self) -> Vec3 {
        self.right
    }

    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    pub fn pitch(&self) -> f32 {
        self.pitch
    }

    pub fn proj_view_matrix(&self) -> Mat4 {
        self.proj_view
    }

    pub fn projection_matrix(&self) -> Mat4 {
        self.projection
    }

    pub fn view_matrix(&self) -> Mat4 {
        self.view
    }

    fn create_projection_matrix(fov_rad: f32, win_size: Vec2) -> Mat4 {
        Mat4::perspective_infinite_reverse_rh(fov_rad, win_size.x / win_size.y, 0.1)
    }
}

fn euler_to_vec(yaw: f32, pitch: f32) -> Vec3 {
    let (yc, ys) = (yaw.cos(), yaw.sin());
    let (pc, ps) = (pitch.cos(), pitch.sin());
    Vec3::new(yc * pc, ps, ys * pc)
}

fn compute_right(facing: Vec3) -> Vec3 {
    facing.cross(Vec3::Y)
}

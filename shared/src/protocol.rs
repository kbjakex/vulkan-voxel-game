use std::f32::consts::{PI, TAU};

use glam::{Vec2, Vec3, vec3, vec2};

pub const PROTOCOL_VERSION: u16 = 0;
pub const PROTOCOL_MAGIC: u16 = 0xB7C1;

pub const MAX_ONLINE_PLAYERS: u16 = 64;

pub type RawNetworkId = u16;

// A per-entity unique identifier shared with all connected clients to identify entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkId(RawNetworkId);

impl NetworkId {
    pub const INVALID : NetworkId = Self::from_raw(0);

    pub const fn from_raw(raw: RawNetworkId) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> RawNetworkId {
        self.0
    }
}

impl std::fmt::Display for NetworkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("NID({})", self.raw()))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MessageError {
    NotEnoughData,
    Malformed, // = kick player
}

// wrap angle into [-PI, PI] range
pub fn wrap_angle(angle: f32) -> f32 {
    let mut angle = angle % TAU; // [-2PI, 2PI]
    if angle < -PI {
        angle += TAU;
    }
    else if angle > PI {
        angle -= TAU;
    }

    angle
}

pub fn wrap_angles(angles: Vec2) -> Vec2 {
    Vec2 {
        x: wrap_angle(angles.x),
        y: wrap_angle(angles.y),
    }
}

/// Input MUST be in range [-PI, PI]. Unexpected outputs otherwise
pub fn encode_angle_rad(angle: f32) -> u16 {
    debug_assert!((-PI..=PI).contains(&angle));
    let mut angle = angle;
    angle += std::f32::consts::PI;
    angle *= 1.0/std::f32::consts::TAU;
    angle *= 65536.0;
    angle.round() as u16
}

// Result always in [-PI, PI]
pub fn decode_angle_rad(encoded: u16) -> f32 {
    let mut encoded = encoded as f32;
    encoded *= 1.0/65536.0;
    encoded *= std::f32::consts::TAU;
    encoded -= std::f32::consts::PI;
    encoded
}

pub fn encode_velocity(coord: f32) -> u32 {
    let signed = ((coord * 2048.0).round() as i32).clamp(-32768, 32767) + 32768;
    if signed < 0 {
        return 0;
    }
    (signed as u32).min(65536)
}

pub fn decode_velocity(coord: u32) -> f32 {
    (coord as i32 - 32768) as f32 / 2048.0
}

pub fn round_velocity(vel: Vec3) -> Vec3 {
    // Simulates the network compression and decompression
    let x = decode_velocity(encode_velocity(vel.x));
    let y = decode_velocity(encode_velocity(vel.y));
    let z = decode_velocity(encode_velocity(vel.z));
    vec3(x, y, z)
}

pub fn round_angles(a: Vec2) -> Vec2 {
    let yaw = decode_angle_rad(encode_angle_rad(wrap_angle(a.x)));
    let pitch = decode_angle_rad(encode_angle_rad(wrap_angle(a.y)));
    vec2(yaw, pitch)
}

mod tests {
    #[test]
    fn test_angles() {
        use super::{decode_angle_rad, encode_angle_rad};
        let angle1 = f32::to_radians(170.0);
        let angle2 = f32::to_radians(-170.0);
        let angle3 = f32::to_radians(-0.0);

        println!("{:10}, {:10}, {:10}", angle1, angle2, angle3);

        let angle1 = decode_angle_rad(encode_angle_rad(angle1));
        let angle2 = decode_angle_rad(encode_angle_rad(angle2));
        let angle3 = decode_angle_rad(encode_angle_rad(angle3));

        println!("{:10}, {:10}, {:10}", angle1, angle2, angle3);

        let angle1 = decode_angle_rad(encode_angle_rad(angle1));
        let angle2 = decode_angle_rad(encode_angle_rad(angle2));
        let angle3 = decode_angle_rad(encode_angle_rad(angle3));

        println!("{:10}, {:10}, {:10}", angle1, angle2, angle3);

        let angle1 = decode_angle_rad(encode_angle_rad(angle1));
        let angle2 = decode_angle_rad(encode_angle_rad(angle2));
        let angle3 = decode_angle_rad(encode_angle_rad(angle3));

        println!("{:10}, {:10}, {:10}", angle1, angle2, angle3);

        let angle1 = decode_angle_rad(encode_angle_rad(angle1));
        let angle2 = decode_angle_rad(encode_angle_rad(angle2));
        let angle3 = decode_angle_rad(encode_angle_rad(angle3));

        println!("{:10}, {:10}, {:10}", angle1, angle2, angle3);
    }

    #[test]
    fn test_angle_roundtrip() {
        use super::{decode_angle_rad, encode_angle_rad, wrap_angle};
        for f in [0.312150524, -1.23412518, 3.141152987, -3.141241898, 0.0, 2.31218427918] {
            let f1 = decode_angle_rad(encode_angle_rad(wrap_angle(f)));
            println!("f {f}, f1 {f1}");
            assert!((f1-f).abs() < 0.0005);

            let f2 = decode_angle_rad(encode_angle_rad(f1));
            println!("f2 {f2}");
            assert_eq!(f1, f2);

            let f3 = decode_angle_rad(encode_angle_rad(f2));
            println!("f3 {f3}");
            assert_eq!(f1, f3);
        }
    }

    #[test]
    fn test_velocity_roundtrip() {
        use super::{decode_velocity, encode_velocity};
        for f in [0.31241825, 0.9128419874, 15.12491874, -4.23147942, 0.512571958] {
            let f1 = decode_velocity(encode_velocity(f));
            println!("f {f}, f1 {f1}");
            assert!((f1-f).abs() < 0.01);

            let f2 = decode_velocity(encode_velocity(f1));
            assert_eq!(f1, f2);

            let f3 = decode_velocity(encode_velocity(f2));
            assert_eq!(f1, f3);
        }
    }
}
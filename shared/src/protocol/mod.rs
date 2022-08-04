pub mod server_to_client;
pub mod client_to_server;

pub use server_to_client as s2c;
pub use client_to_server as c2s;

use std::f32::consts::PI;

pub(crate) const PROTOCOL_VERSION: u16 = 0;
pub(crate) const PROTOCOL_MAGIC: u16 = 0xB7C1;

pub type RawNetworkId = u16;

// A per-entity unique identifier shared with all connected clients to identify entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkId(RawNetworkId);

impl NetworkId {
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

const ANGLE_ENCODE_CONSTANT : f64 = (1 << 15) as f64 / std::f64::consts::TAU;

/// Input MUST be in range [-PI, PI]. Unexpected outputs otherwise
pub fn encode_angle_rad(angle: f32) -> u16 {
    debug_assert!((-PI..=PI).contains(&angle));

    (angle * ANGLE_ENCODE_CONSTANT as f32) as i16 as u16
}

// Result always in [-PI, PI]
pub fn decode_angle_rad(encoded: u16) -> f32 {
    (encoded as i16 as f64 * (1.0 / ANGLE_ENCODE_CONSTANT)) as f32
}

mod tests {
    
    #[test]
    fn test_angles() {
        use super::*;
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
}
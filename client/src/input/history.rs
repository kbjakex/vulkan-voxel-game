use std::f32::consts::PI;

use glam::Vec3;
use shared::protocol;

use super::{Keyboard, Mouse, settings::InputSettings};

const LEFT_BIT : u32 = 0;
const RIGHT_BIT : u32 = 1;
const FWD_BIT : u32 = 2;
const BACK_BIT : u32 = 3;
const UP_BIT : u32 = 4;
const DOWN_BIT : u32 = 5;

pub const LEFT : u32 = 1 << LEFT_BIT;
pub const RIGHT : u32 = 1 << RIGHT_BIT;
pub const FWD : u32 = 1 << FWD_BIT;
pub const BACK : u32 = 1 << BACK_BIT;
pub const UP : u32 = 1 << UP_BIT;
pub const DOWN : u32 = 1 << DOWN_BIT;

#[derive(Default, Clone, Copy)]
pub struct PlayerStateSnapshot {
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32
}

#[derive(Clone, Copy)]
pub struct InputSnapshot {
    pub keys: u8,
    pub yaw_delta: u16, // 16 bits, approx 1/180th of a degree
    pub pitch_delta: u16, // 16 bits
}

impl InputSnapshot {
    pub fn take(kb: &Keyboard, mouse: &Mouse, settings: &InputSettings) -> Self {
        let [delta_x, delta_y] = mouse.pos_delta().to_array();
        let delta_x = protocol::encode_angle_rad(delta_x);
        let delta_y = protocol::encode_angle_rad(delta_y);

        let bindings = &settings.key_bindings;

        let mut bitset = 0u8;
        bitset |= (kb.pressed(bindings.fwd) as u8) << FWD_BIT;
        bitset |= (kb.pressed(bindings.back) as u8) << BACK_BIT;
        bitset |= (kb.pressed(bindings.left) as u8) << LEFT_BIT;
        bitset |= (kb.pressed(bindings.right) as u8) << RIGHT_BIT;
        bitset |= (kb.pressed(bindings.jump) as u8) << UP_BIT;

        Self {
            keys: bitset,
            yaw_delta: delta_x,
            pitch_delta: delta_y,
        }
    }

    pub fn simulate_on(&self, mut state: PlayerStateSnapshot) -> PlayerStateSnapshot {
        state.yaw += protocol::decode_angle_rad(self.yaw_delta);
        state.pitch += protocol::decode_angle_rad(self.pitch_delta);
        state.pitch = state.pitch.clamp(-PI/2.0, PI/2.0);

        let right = ((self.keys >> RIGHT_BIT) & 1) as i32 - ((self.keys >> LEFT_BIT) & 1) as i32;
        let fwd = ((self.keys >> FWD_BIT) & 1) as i32 - ((self.keys >> BACK_BIT) & 1) as i32;
        let up = ((self.keys >> UP_BIT) & 1) as i32 - ((self.keys >> DOWN_BIT) & 1) as i32;

        if right != 0 || up != 0 || fwd != 0 {
            let fwd_dir = Vec3::new(
                state.yaw.cos(),
                state.pitch.sin(),
                state.yaw.sin()
            );
            let up_dir = Vec3::Y;
            let right_dir = fwd_dir.cross(up_dir);
    
            let velocity = (right as f32) * right_dir + (fwd as f32) * fwd_dir + (up as f32) * up_dir;
            state.pos += velocity.normalize() * 0.15;
        }

        state
    }
}

const MAX_RING_BUF_ENTRIES : usize = 32; // 30 gameticks = one second. Should never take this long to get a response

pub struct InputSnapshotBuffer {
    buffer: [InputSnapshot; MAX_RING_BUF_ENTRIES], // ring buffer
    start_idx: usize,
    size: usize,

    oldest_gametick: u32,
}

impl Default for InputSnapshotBuffer {
    fn default() -> Self {
        Self {
            buffer: [InputSnapshot { keys: 0, yaw_delta: 0, pitch_delta: 0 }; 32],
            start_idx: 0,
            size: 0,
            oldest_gametick: 0,
        }
    }
}

impl InputSnapshotBuffer {
    pub fn push_new_snapshot(&mut self, snapshot: InputSnapshot) {
        if self.size == MAX_RING_BUF_ENTRIES {
            self.buffer[self.start_idx] = snapshot;
            self.start_idx += 1;
            self.start_idx %= MAX_RING_BUF_ENTRIES;
        } else {
            self.buffer[(self.start_idx + self.size) % MAX_RING_BUF_ENTRIES] = snapshot;
            self.size += 1;
        }
    }

    pub fn get_by_gametick(&self, gametick: u32) -> Option<InputSnapshot> {
        if gametick < self.oldest_gametick {
            None
        } else {
            self.buffer.get((self.start_idx + (gametick - self.oldest_gametick) as usize) % MAX_RING_BUF_ENTRIES).cloned()
        }
    }

    pub fn drop_all_before_gametick(&mut self, gametick: u32) {
        if gametick < self.oldest_gametick {
            return;
        }

        let num_to_drop = ((gametick - self.oldest_gametick) as usize).min(self.size);
        self.start_idx += num_to_drop;
        self.start_idx %= MAX_RING_BUF_ENTRIES;

        self.size -= num_to_drop;

        self.oldest_gametick = gametick;
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn iter(&self) -> Snapshots {
        Snapshots {
            buffer: &self.buffer,
            pos: self.start_idx,
            left: self.size,
        }
    }
}

pub struct Snapshots<'a> {
    buffer: &'a [InputSnapshot],
    pos: usize,
    left: usize,
}

impl<'a> Iterator for Snapshots<'a> {
    type Item = InputSnapshot;

    fn next(&mut self) -> Option<Self::Item> {
        if self.left > 0 {
            self.left -= 1;
            self.pos += 1;
            Some(self.buffer[(self.pos - 1) % MAX_RING_BUF_ENTRIES])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push() {
        const TEST_VALS : [u8; 5] = [3, 1, 5, 2, 8];

        let mut buf = InputSnapshotBuffer::default();

        assert_eq!(buf.len(), 0);

        for x in TEST_VALS {
            buf.push_new_snapshot(InputSnapshot {
                keys: x,
                yaw_delta: 0,
                pitch_delta: 0,
            });
        }

        assert_eq!(buf.len(), TEST_VALS.len());

        for (a, &b) in buf.iter().zip(TEST_VALS.iter()) {
            assert_eq!(a.keys, b);
        }
    }

    #[test]
    fn test_overflow() {
        let mut buf = InputSnapshotBuffer::default();

        for i in 0..MAX_RING_BUF_ENTRIES {
            buf.push_new_snapshot(InputSnapshot {
                keys: i as _,
                yaw_delta: 0,
                pitch_delta: 0,
            });
        }
        assert_eq!(buf.iter().next().map(|s| s.keys), Some(0));
        assert_eq!(buf.iter().nth(1).map(|s| s.keys), Some(1));

        buf.push_new_snapshot(InputSnapshot {
            keys: 32,
            yaw_delta: 0,
            pitch_delta: 0,
        });
        assert_eq!(buf.iter().next().map(|s| s.keys), Some(1));
        assert_eq!(buf.iter().last().map(|s| s.keys), Some(32));

        buf.push_new_snapshot(InputSnapshot {
            keys: 33,
            yaw_delta: 0,
            pitch_delta: 0,
        });
        assert_eq!(buf.iter().next().map(|s| s.keys), Some(2));
        assert_eq!(buf.iter().nth(1).map(|s| s.keys), Some(3));
        assert_eq!(buf.iter().last().map(|s| s.keys), Some(33));
    }

    #[test]
    fn test_gameticks() {


    }
}
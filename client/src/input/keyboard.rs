use std::vec::Drain;

use winit::event::{VirtualKeyCode, DeviceEvent, WindowEvent, KeyboardInput, ElementState};


const CTRL_BIT: usize = 0;
const LSHIFT_BIT: usize = 1;
const ALT_BIT: usize = 2;

pub const CTRL: u32 = 1 << (CTRL_BIT as u32);
pub const LSHIFT: u32 = 1 << (LSHIFT_BIT as u32);
pub const ALT: u32 = 1 << (ALT_BIT as u32);

pub struct Keyboard {
    pressed: Vec<u32>,              // index -> "frame count when pressed & 0xFFFF"
    just_released: Vec<(u32, u32)>, // index -> ("number of frames pressed", "frame count when released")
    frame_counter: u32,

    text_mode: bool,
    text_buffer: Vec<char>,
    ignore_next: bool, // ignore next window event after entering text mode because of duplicated events
}

pub type Key = VirtualKeyCode;

impl Keyboard {
    pub fn enter_text_input_mode(&mut self) {
        if self.is_in_text_input_mode() {
            panic!("Overlapping 'enter_text_input_mode', was already in text mode!");
        }
        self.text_mode = true;
        self.ignore_next = true;
        self.text_buffer.clear();
    }

    pub fn is_in_text_input_mode(&self) -> bool {
        self.text_mode
    }

    pub fn exit_text_input_mode(&mut self) {
        self.text_mode = false;
        self.text_buffer.clear();
    }

    pub fn drain_text_inputs(&mut self) -> Drain<char> {
        self.text_buffer.drain(..)
    }

    pub fn has_text_inputs(&self) -> bool {
        !self.text_buffer.is_empty()
    }

    pub fn get_axis(&self, positive_key: Key, negative_key: Key) -> i32 {
        self.pressed(positive_key) as i32 - self.pressed(negative_key) as i32
    }

    pub fn pressed(&self, key: Key) -> bool {
        self.pressed_frames(key) > 0
    }

    pub fn pressed_frames(&self, key: Key) -> u32 {
        let timestamp = self.pressed[key as usize];
        if timestamp == 0 {
            0
        } else {
            self.frame_counter - timestamp
        }
    }

    pub fn pressed_with_mods(&self, key: Key, mods: u32) -> bool {
        self.pressed_frames_with_mods(key, mods) > 0
    }

    pub fn pressed_frames_with_mods(&self, key: Key, mods: u32) -> u32 {
        let ticks_down = self.pressed_frames(key);
        if ticks_down == 0 {
            return 0;
        }
        // Logic here is that you usually have to press a modifier key *before* you press
        // the key you want to apply it to. You wouldn't press 'S + ctrl' to save, but 'ctrl + S'.
        // Therefore I'm requiring the modifiers to have been held down longer than the key.
        if (mods & CTRL) != 0 && self.pressed_frames(Key::LControl) < ticks_down {
            return 0;
        }
        if (mods & ALT) != 0 && self.pressed_frames(Key::LAlt) < ticks_down {
            return 0;
        }
        if (mods & LSHIFT) != 0 && self.pressed_frames(Key::LShift) < ticks_down {
            return 0;
        }
        ticks_down
    }

    pub fn just_pressed(&self, key: Key) -> bool {
        self.pressed_frames(key) == 1
    }

    pub fn just_pressed_with_mods(&self, key: Key, mods: u32) -> bool {
        self.pressed_frames_with_mods(key, mods) == 1
    }

    pub fn tapped(&self, key: Key) -> bool {
        self.tapped_with_threshold(key, 7)
    }

    pub fn tapped_with_threshold(&self, key: Key, max_frames: u32) -> bool {
        self.just_released_frames(key) <= max_frames
    }

    pub fn just_released(&self, key: Key) -> bool {
        self.just_released_frames(key) > 0
    }

    pub fn just_released_frames(&self, key: Key) -> u32 {
        let (frame_count, check) = self.just_released[key as usize];
        if check != self.frame_counter {
            0
        } else {
            frame_count
        }
    }

    pub fn release(&mut self, key: Key) -> bool {
        self.release_get_frames(key) > 0
    }

    /// Gets the number of frames the key has been pressed, or 0 if it none
    pub fn release_get_frames(&mut self, key: Key) -> u32 {
        let frames = self.pressed_frames(key);
        self.pressed[key as usize] = 0;
        frames
    }
}

pub struct KeyboardUpdater;

impl KeyboardUpdater {
    pub fn new_keyboard() -> Keyboard {
        let mut pressed = Vec::new();
        pressed.resize(256, 0);

        let mut just_released = Vec::new();
        just_released.resize(256, (0, 0));

        Keyboard {
            pressed,
            just_released,
            frame_counter: 0,
            text_mode: false,
            text_buffer: Vec::new(),
            ignore_next: false,
        }
    }

    // Returns false if event not consumed
    pub fn handle_key_event(event: &DeviceEvent, keyboard: &mut Keyboard) -> bool {
        match event {
            DeviceEvent::Key(input) if !keyboard.is_in_text_input_mode() => {
                Self::handle_key_press(input, keyboard);
            }
            _ => return false,
        }
        true
    }

    pub fn handle_window_event(event: &WindowEvent, keyboard: &mut Keyboard) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                device_id: _,
                input,
                ..
            } if keyboard.is_in_text_input_mode() => {
                if keyboard.ignore_next {
                    keyboard.ignore_next = false;
                    return true;
                }
                Self::handle_key_press(input, keyboard);
            },
            WindowEvent::ReceivedCharacter(char) => {
                if keyboard.is_in_text_input_mode() {
                    keyboard.text_buffer.push(*char);
                }
            }
            _ => return false,
        }
        true
    }

    fn handle_key_press(input: &KeyboardInput, keyboard: &mut Keyboard) {
        let key = match input.virtual_keycode {
            Some(key) => key,
            None => {
                return;
            }
        };

        match input.state {
            ElementState::Pressed => {
                // Winit does not distinguish between 'Pressed' and 'Repeat',
                // and frame counting breaks if repeat is not filtered out, so
                // check first that the key has actually been released before re-assigning.
                // Allow repeat in text mode though
                if keyboard.is_in_text_input_mode() || keyboard.pressed[key as usize] == 0 {
                    keyboard.pressed[key as usize] = keyboard.frame_counter;
                }
            }
            ElementState::Released => {
                let frames_pressed = keyboard.pressed_frames(key);
                keyboard.pressed[key as usize] = 0;
                keyboard.just_released[key as usize] = (frames_pressed, keyboard.frame_counter);
            }
        }
    }

    pub fn tick_keyboard(keyboard: &mut Keyboard) {
        keyboard.frame_counter += 1;
    }
}
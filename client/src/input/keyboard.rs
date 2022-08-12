use winit::event::{DeviceEvent, ElementState, KeyboardInput, ModifiersState, VirtualKeyCode};

pub type Mods = ModifiersState;

pub struct Keyboard {
    pressed: Box<[u32]>,              // index -> "frame count when pressed & 0xFFFF"
    just_released: Box<[(u32, u32)]>, // index -> ("number of frames pressed", "frame count when released")
    frame_counter: u32,
}

pub type Key = VirtualKeyCode;

impl Keyboard {
    pub fn clear_all(&mut self) {
        self.pressed.fill(0);
        self.just_released.fill((0, 0));
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

    pub fn pressed_with_mods(&self, key: Key, mods: Mods) -> bool {
        self.pressed_frames_with_mods(key, mods) > 0
    }

    pub fn pressed_frames_with_mods(&self, key: Key, mods: Mods) -> u32 {
        let ticks_down = self.pressed_frames(key);
        if ticks_down == 0 {
            return 0;
        }
        // Logic here is that you usually have to press a modifier key *before* you press
        // the key you want to apply it to. You wouldn't press 'S + ctrl' to save, but 'ctrl + S'.
        // Therefore I'm requiring the modifiers to have been held down longer than the key.
        if mods.ctrl() && self.pressed_frames(Key::LControl) < ticks_down {
            return 0;
        }
        if mods.alt() && self.pressed_frames(Key::LAlt) < ticks_down {
            return 0;
        }
        if mods.shift() && self.pressed_frames(Key::LShift) < ticks_down {
            return 0;
        }
        ticks_down
    }

    pub fn just_pressed(&self, key: Key) -> bool {
        self.pressed_frames(key) == 1
    }

    pub fn just_pressed_with_mods(&self, key: Key, mods: Mods) -> bool {
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

impl Keyboard {
    pub fn new() -> Self {
        let mut pressed = Vec::new();
        pressed.resize(256, 0);

        let mut just_released = Vec::new();
        just_released.resize(256, (0, 0));

        Self {
            pressed: pressed.into_boxed_slice(),
            just_released: just_released.into_boxed_slice(),
            frame_counter: 0,
        }
    }

    // Returns false if event not consumed
    pub fn handle_key_event(keyboard: &mut Keyboard, event: &DeviceEvent) -> bool {
        if let &DeviceEvent::Key(KeyboardInput {
            virtual_keycode: Some(key),
            state,
            ..
        }) = event
        {
            match state {
                ElementState::Pressed => {
                    // Winit does not distinguish between 'Pressed' and 'Repeat',
                    // and frame counting breaks if repeat is not filtered out, so
                    // check first that the key has actually been released before re-assigning.
                    // Allow repeat in text mode though
                    if keyboard.pressed[key as usize] == 0 {
                        keyboard.pressed[key as usize] = keyboard.frame_counter;
                    }
                }
                ElementState::Released => {
                    let frames_pressed = keyboard.pressed_frames(key);
                    keyboard.pressed[key as usize] = 0;
                    keyboard.just_released[key as usize] = (frames_pressed, keyboard.frame_counter);
                }
            }
            return true;
        }
        false
    }

    pub fn tick(keyboard: &mut Keyboard) {
        keyboard.frame_counter += 1;
    }
}

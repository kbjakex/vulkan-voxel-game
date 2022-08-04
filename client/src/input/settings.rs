use super::Key;

#[derive(Debug)]
pub struct Keybindings {
    pub fwd: Key,
    pub left: Key,
    pub right: Key,
    pub back: Key,
    pub jump: Key,
    pub open_chat: Key,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            fwd: Key::W,
            left: Key::A,
            right: Key::D,
            back: Key::S,
            jump: Key::Space,
            open_chat: Key::Return,
        }
    }
}

#[derive(Debug)]
pub struct InputSettings {
    pub key_bindings: Keybindings,
    pub mouse_sensitivity: f32,
}

impl Default for InputSettings {
    fn default() -> Self {
        Self {
            key_bindings: Keybindings::default(),
            mouse_sensitivity: 1.0,
        }
    }
}

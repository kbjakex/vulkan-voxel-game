pub mod keyboard;
pub mod mouse;
pub mod settings;

use arboard::Clipboard;
use glam::Vec2;
pub use keyboard::*;
pub use mouse::*;
use winit::event::{Event, ModifiersState, WindowEvent};

use crate::resources;

use self::settings::InputSettings;

pub fn init(wnd_size: (u32, u32)) -> anyhow::Result<resources::input::Resources> {
    Ok(resources::input::Resources {
        settings: InputSettings::default(),
        mouse: Mouse::new(Vec2::new(wnd_size.0 as f32 / 2.0, wnd_size.1 as f32 / 2.0)),
        keyboard: Keyboard::new(),
        clipboard: Clipboard::new()?,
        keyboard_mods: ModifiersState::empty(),
    })
}

// Returns true if event was consumed
pub fn handle_event(event: &Event<()>, res: &mut resources::input::Resources) -> bool {
    match &event {
        Event::DeviceEvent { event, .. } => {
            return Keyboard::handle_key_event(&mut res.keyboard, event)
        }
        Event::WindowEvent { event, .. } => {
            Mouse::handle_mouse_events(&mut res.mouse, event);

            if let WindowEvent::ModifiersChanged(mods) = event {
                res.keyboard_mods = *mods;
                return true;
            }
        }
        _ => {}
    }
    false
}

pub mod history;
pub mod keyboard;
pub mod mouse;
pub mod settings;

use arboard::Clipboard;
pub use keyboard::*;
pub use mouse::*;
use winit::dpi::LogicalSize;

use crate::resources;

use self::settings::InputSettings;

pub fn init(wnd_size: LogicalSize<u32>) -> anyhow::Result<resources::input::Resources> {
    Ok(resources::input::Resources {
        settings: InputSettings::default(),
        mouse: MouseUpdater::new_mouse(wnd_size),
        keyboard: KeyboardUpdater::new_keyboard(),
        clipboard: Clipboard::new()?
    })
}

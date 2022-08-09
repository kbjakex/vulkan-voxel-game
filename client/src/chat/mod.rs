pub mod commands;

use std::sync::Mutex;

use arboard::Clipboard;
use glam::Vec2;
use hecs::Entity;
use once_cell::sync::Lazy;
use smallvec::SmallVec;
use winit::{window::Window, dpi::LogicalPosition};

use crate::{
    input::{Key, Keyboard, Mouse},
    networking::Connection,
    renderer::{
        text_renderer::{ColorRange, Style, TextColor, TextRenderer},
        ui_renderer::UiRenderer,
    },
    resources::core::WindowSize,
    text_box::{TextBox, TextBoxBuilder},
};

static CHAT_QUEUE: Lazy<Mutex<Vec<(String, TextColor)>>> = Lazy::new(|| Mutex::new(Vec::new()));

struct LineBreaks {
    max_width_px: u16,           // to check if the indices are outdated
    indices: SmallVec<[u16; 4]>, // byte positions
}

struct ChatEntry {
    contents: String,
    color: TextColor,
    time_received: f32,
    sender: Option<Entity>,
    linebreaks: LineBreaks,
}

struct ChatHistory {
    entries: Box<[Option<ChatEntry>; 256]>,
    head: usize,
}

impl ChatHistory {
    pub fn new() -> Self {
        Self {
            entries: Box::new([(); 256].map(|_| None)),
            head: 0,
        }
    }

    pub fn add_entry(&mut self, entry: ChatEntry) {
        self.head = self.head.wrapping_sub(1) % 256;
        self.entries[self.head] = Some(entry);
        self.entries[self.head.wrapping_sub(1) % 256] = None;
    }
}

pub struct Chat {
    // Contains entries that are drawn in chat, including own messages,
    // but no commands
    history: ChatHistory,
    own_messages: Vec<Vec<char>>, // includes commands!
    unprocessed_commands: Vec<String>,

    chat_open: bool,
    text_box: TextBox,

    // for scrolling up and down own messages
    message_browser_idx: usize,
}

impl Chat {
    pub fn write(message: String, color_rgba: u32) {
        println!("{message}");
        CHAT_QUEUE
            .lock()
            .unwrap()
            .push((message, TextColor::from_rgba32(color_rgba)));
    }

    pub fn new(win_width: u16) -> Self {
        let text_box = TextBoxBuilder::new_at(10, 12)
            .with_length_limit(500)
            .with_width(win_width - 20)
            .build();

        Self {
            history: ChatHistory::new(),
            own_messages: Vec::new(),
            unprocessed_commands: Vec::new(),
            chat_open: false,
            text_box,
            message_browser_idx: usize::MAX,
        }
    }

    pub fn add_chat_entry(
        &mut self,
        sender: Option<Entity>,
        message: String,
        color: TextColor,
        time_received: f32,
    ) {
        self.history.add_entry(ChatEntry {
            sender,
            contents: message,
            color,
            time_received,
            linebreaks: LineBreaks {
                max_width_px: u16::MAX,
                indices: SmallVec::new(),
            }, // uncomputed
        });
    }

    pub fn on_window_resize(&mut self, new_width: u16, renderer: &TextRenderer) {
        self.text_box.set_width(new_width - 20, renderer);
    }

    pub fn toggle_open(&mut self, keyboard: &mut Keyboard, window: &Window, window_size: Vec2) -> anyhow::Result<()> {
        if self.chat_open {
            println!("Closing chat");
            self.chat_open = false;

            self.text_box.reset();
            self.message_browser_idx = usize::MAX;
            
            keyboard.exit_text_input_mode();
    
            window.set_cursor_position(LogicalPosition::new(window_size.x as i32 / 2, window_size.y as i32 / 2))?;
            window.set_cursor_grab(winit::window::CursorGrabMode::Confined)?;
            window.set_cursor_visible(false);
        } else {
            self.chat_open = true;
            println!("Entering text input mode");

            keyboard.enter_text_input_mode();

            window.set_cursor_position(LogicalPosition::new(window_size.x as i32 / 2, window_size.y as i32 / 2))?;
            window.set_cursor_grab(winit::window::CursorGrabMode::None)?;
            window.set_cursor_visible(true);
        }
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.chat_open
    }
}

pub fn draw_chat(
    chat: &mut Chat,
    time_secs: f32,
    renderer: &mut UiRenderer,
    win_size: &WindowSize,
) {
    let max_time_ago = if chat.chat_open { f32::MAX } else { 10.0 };
    let mut y = 26;

    let max_width_px = (win_size.extent.width * 4 / 10).max(384) as u16;
    let max_height_px = 767 + y;

    let mut lines_drawn = 0;

    let mut idx = chat.history.head;
    while let Some(entry) = &mut chat.history.entries[idx] {
        idx = (idx + 1) % 256;

        if y >= max_height_px || time_secs - entry.time_received > max_time_ago {
            break;
        }

        let linebreaks = &mut entry.linebreaks;

        if linebreaks.max_width_px != max_width_px {
            // Outdated, recompute
            linebreaks.max_width_px = max_width_px;
            linebreaks.indices = renderer
                .text()
                .compute_linebreaks(&entry.contents, max_width_px);
        }

        y += linebreaks.indices.len() as u16 * 30;

        let mut line_y = y;

        let mut start_idx = 0;
        for end_idx in linebreaks.indices.iter().copied() {
            let line = &entry.contents[start_idx as usize..end_idx as usize];

            renderer.text().draw_2d(
                line,
                16,
                line_y,
                Style {
                    colors: &[ColorRange::new(entry.color, u32::MAX)],
                    ..Default::default()
                },
            );

            lines_drawn += 1;

            start_idx = end_idx;
            line_y -= 30;
        }
    }

    if lines_drawn != 0 {
        const PAD: u16 = 2 * 3; // 3 is the scale
        renderer.draw_rect_xy_wh(
            (16 - PAD, 56 - PAD),
            (
                max_width_px + 2 * PAD,
                lines_drawn as u16 * 30 + 2 * PAD - 10,
            ),
            0x06_06_06_50,
        );
    }
}

pub fn process_text_input(
    keyboard: &mut Keyboard,
    mouse: &mut Mouse,
    chat: &mut Chat,
    ui_renderer: &mut UiRenderer,
    conn: &mut Connection,
    clipboard: &mut Clipboard,
    win_size: &WindowSize,
    window: &Window,
    time_secs: f32,
) -> anyhow::Result<()> {
    if !keyboard.is_in_text_input_mode() && keyboard.release(Key::Return) && !chat.is_open() {
        chat.toggle_open(keyboard, window, win_size.xy)?;
    }

    CHAT_QUEUE
        .lock()
        .unwrap()
        .drain(..)
        .for_each(|(msg, color)| {
            chat.add_chat_entry(None, msg, color, time_secs);
        });

    if !chat.chat_open {
        return Ok(());
    }

    // +1 if up pressed, 0 if both pressed, -1 if down pressed
    let scroll = keyboard.pressed(Key::Up) as i32 - keyboard.pressed(Key::Down) as i32;

    chat.text_box.process_inputs(
        keyboard,
        mouse,
        clipboard,
        ui_renderer.text(),
        win_size.extent.height as _,
        time_secs,
    );

    if scroll != 0 && (!chat.text_box.modified() || chat.text_box.contents().is_empty()) {
        keyboard.release(Key::Up);
        keyboard.release(Key::Down);

        if chat.message_browser_idx == usize::MAX {
            if scroll > 0 && !chat.own_messages.is_empty() {
                chat.message_browser_idx = 0;
            }
        } else if scroll < 0 && chat.message_browser_idx == 0 {
            chat.message_browser_idx = usize::MAX;
        } else if !chat.own_messages.is_empty() {
            let history_idx = (chat.message_browser_idx as i32 + scroll)
                .clamp(0, chat.own_messages.len() as i32 - 1)
                as usize;
            chat.message_browser_idx = history_idx;
        }

        if chat.message_browser_idx != usize::MAX {
            chat.text_box.set_contents(
                chat.own_messages[chat.own_messages.len() - 1 - chat.message_browser_idx].clone(),
                ui_renderer.text(),
            );
        } else {
            chat.text_box.set_contents(Vec::new(), ui_renderer.text());
        }
    } else if chat.text_box.modified() {
        chat.message_browser_idx = usize::MAX;
    }

    let w = win_size.xy.x as u16;

    ui_renderer.draw_rect_xy_wh(
        (10 - 2 * 3, 12 - 2 * 3),
        (w - 20 + 2 * 3, 10 * 3),
        0x06_06_06_50,
    );
    chat.text_box
        .draw(ui_renderer, win_size.extent.height as _, time_secs);

    let tbox = &mut chat.text_box;

    if keyboard.release(Key::Return) {
        let contents = trim_message(tbox.contents());
        if !contents.is_empty() {
            chat.own_messages.push(contents.to_owned());

            if contents[0] == '/' {
                chat.unprocessed_commands.push(contents.iter().collect());
            } else if let Some(channels) = conn.channels() {
                if channels.chat_send.send(contents.iter().collect()).is_err() {
                    chat.add_chat_entry(
                        None,
                        "Failed to send message".to_owned(),
                        TextColor::from_rgba32(0xFF_00_00_FF),
                        time_secs,
                    );
                }
            } else {
                let msg = format!("(Disconnected) {}", contents.iter().collect::<String>());
                chat.add_chat_entry(None, msg, TextColor::from_rgba32(0x77_77_77_77), time_secs);
            }
        }

        chat.toggle_open(keyboard, window, win_size.xy)?;
    }
    Ok(())
}

fn trim_message(mut msg: &[char]) -> &[char] {
    while msg.first() == Some(&' ') {
        msg = &msg[1..];
    }

    while msg.last() == Some(&' ') {
        msg = &msg[..msg.len() - 1];
    }

    msg
}

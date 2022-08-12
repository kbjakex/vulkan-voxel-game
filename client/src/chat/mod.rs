use flexstr::LocalStr;
use glam::Vec2;
use smallvec::SmallVec;
use winit::{
    dpi::LogicalPosition,
    event::{ElementState, KeyboardInput, WindowEvent},
    window::{CursorGrabMode, Window},
};

use crate::{
    input::Key,
    networking::Connection,
    renderer::{
        text_renderer::{ColorRange, Style, TextColor},
        ui_renderer::UiRenderer,
    },
    resources::{core::WindowSize, Resources},
    text_box::{TextBox, TextBoxBuilder},
};

struct LineBreaks {
    max_width_px: u16,           // to check if the indices are outdated
    indices: SmallVec<[u16; 4]>, // byte positions
}

struct ChatEntry {
    contents: LocalStr,
    color: TextColor,
    time_received: f32,
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

    chat_open: bool,
    text_box: TextBox,

    // for scrolling up and down own messages
    message_browser_idx: Option<usize>,
}

impl Chat {
    pub fn new(win_width: u16) -> Self {
        let text_box = TextBoxBuilder::new_at(10, 12)
            .with_length_limit(500)
            .with_width(win_width - 20)
            .build();

        Self {
            history: ChatHistory::new(),
            own_messages: Vec::new(),
            chat_open: false,
            text_box,
            message_browser_idx: None,
        }
    }

    pub fn add_chat_entry(&mut self, message: LocalStr, color: TextColor, time_received: f32) {
        self.history.add_entry(ChatEntry {
            contents: message,
            color,
            time_received,
            linebreaks: LineBreaks {
                max_width_px: u16::MAX,
                indices: SmallVec::new(),
            }, // uncomputed
        });
    }

    pub fn toggle_open(&mut self, window: &Window, window_size: &WindowSize) {
        if self.chat_open {
            self.chat_open = false;

            self.text_box.reset();
            self.message_browser_idx = None;

            Self::set_grab_and_center(window, window_size.xy, CursorGrabMode::Confined);
            window.set_cursor_visible(false);
        } else {
            self.chat_open = true;

            Self::set_grab_and_center(window, window_size.xy, CursorGrabMode::None);
            window.set_cursor_visible(true);
        }
    }

    pub fn is_open(&self) -> bool {
        self.chat_open
    }

    fn set_grab_and_center(wnd: &Window, win_size: Vec2, grab: CursorGrabMode) {
        if let Err(e) = wnd.set_cursor_position::<LogicalPosition<u32>>(
            (win_size / 2.0).as_uvec2().to_array().into(),
        ) {
            println!("Failed to set cursor position: {e}");
        }
        if let Err(e) = wnd.set_cursor_grab(grab) {
            println!("Failed to grab the cursor: {e}");
        }
    }
}

impl Chat {
    // Returns true if event was consumed
    pub fn process_event(
        &mut self,
        event: &WindowEvent,
        res: &mut Resources,
        connection: &mut Connection,
    ) -> bool {
        if !self.is_open() {
            return false;
        }

        match event {
            WindowEvent::Resized(new_size) => {
                self.text_box.set_width(
                    (new_size.width as u16).saturating_sub(20),
                    res.renderer.ui.text(),
                );
                false
            }
            &WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        virtual_keycode: Some(Key::Down),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if !self.own_messages.is_empty()
                && (!self.text_box.modified() || self.text_box.is_empty()) =>
            {
                if let Some(idx) = self.message_browser_idx.as_mut() {
                    *idx = (*idx + 1).min(self.own_messages.len());
                    if *idx == self.own_messages.len() {
                        self.text_box.set_contents(&[], res.renderer.ui.text());
                    } else {
                        self.text_box
                            .set_contents(&self.own_messages[*idx], res.renderer.ui.text());
                    }
                }
                true
            }

            &WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        virtual_keycode: Some(Key::Up),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if !self.own_messages.is_empty()
                && (!self.text_box.modified() || self.text_box.is_empty()) =>
            {
                if let Some(idx) = self.message_browser_idx.as_mut() {
                    *idx = (*idx).saturating_sub(1);
                } else {
                    self.message_browser_idx = Some(self.own_messages.len() - 1);
                }
                self.text_box.set_contents(
                    &self.own_messages[self.message_browser_idx.unwrap()],
                    res.renderer.ui.text(),
                );
                true
            }
            &WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        virtual_keycode: Some(Key::Return),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                let contents = trim_message(self.text_box.contents());
                if !contents.is_empty() {
                    self.own_messages.push(contents.to_owned());

                    if let Some(channels) = connection.channels() && channels.chat.send(contents.iter().collect()).is_ok() {
                        // Success
                    } else {
                        self.add_chat_entry(
                            "Failed to send message".into(),
                            0xFF_00_00_FF.into(),
                            res.time.secs_f32,
                        );
                    }
                }
                let _ = self.toggle_open(&res.window_handle, &res.window_size);
                true
            }
            event => {
                let consumed = self.text_box.process_event(event, res);
                if self.text_box.modified() {
                    self.message_browser_idx = None;
                }
                consumed
            }
        }
    }

    pub fn draw(&mut self, time_secs: f32, renderer: &mut UiRenderer, win_size: &WindowSize) {
        if self.is_open() {
            let w = win_size.extent.width as u16;
            renderer.draw_rect_xy_wh(
                (10 - 2 * 3, 12 - 2 * 3),
                (w - 20 + 2 * 3, 10 * 3),
                0x06_06_06_50,
            );
            self.text_box
                .draw(renderer, win_size.extent.height as _, time_secs);
        }

        let max_time_ago = if self.chat_open { f32::MAX } else { 10.0 };
        let mut y = 26;

        let max_width_px = (win_size.extent.width * 4 / 10).max(384) as u16;
        let max_height_px = 767 + y;

        let mut lines_drawn = 0;

        let mut idx = self.history.head;
        while let Some(entry) = &mut self.history.entries[idx] {
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

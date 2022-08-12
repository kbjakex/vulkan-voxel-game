const DEFAULT_VALID_INPUT_CHARS : &str = " abcdefghijklmnopqrstuvwxyzåäöABCDEFGHIJKLMNOPQRSTUVWXYZÅÄÖ0123456789!\"#¤%&/()=\\?@£€${[]}^¨*+'-;:_,.<>|§";
const CTRL_SEL_STOPPERS: &str = " \t\n.,_-:"; // all only if they're not followed by whitespace

const BACKSPACE: char = '\x08';

use arboard::Clipboard;
use bevy_utils::HashSet;
use winit::event::{ElementState, KeyboardInput, ModifiersState, MouseButton, WindowEvent};

use crate::{
    input::Key,
    renderer::{
        text_renderer::{self, ColorRange, TextColor, TextRenderer},
        ui_renderer::UiRenderer,
    },
    resources::Resources,
};

pub struct TextBoxBuilder {
    valid_chars: Option<HashSet<char>>,
    length_limit: usize,
    x: u16,
    y: u16,
    width: u16,
}

impl TextBoxBuilder {
    pub const fn new_at(x: u16, y: u16) -> Self {
        Self {
            valid_chars: None,
            length_limit: usize::MAX,
            x,
            y,
            width: u16::MAX,
        }
    }

    pub const fn with_width(mut self, width: u16) -> Self {
        self.width = width;
        self
    }

    pub fn with_valid_chars(mut self, chars: HashSet<char>) -> Self {
        self.valid_chars = Some(chars);
        self
    }

    pub const fn with_length_limit(mut self, limit: usize) -> Self {
        self.length_limit = limit;
        self
    }

    pub fn build(self) -> TextBox {
        let valid_chars = self
            .valid_chars
            .unwrap_or_else(|| DEFAULT_VALID_INPUT_CHARS.chars().collect());

        TextBox {
            buffer: Vec::with_capacity(self.length_limit.min(256) as usize),
            old_cursor_pos: 0,
            cursor_pos: 0,
            last_keypress: 0.0,
            valid_chars,
            selection: Selection { start: 0, end: 0 },
            length_limit: self.length_limit,
            mouse_clicks: 0,
            last_mouse_click: 0.0,
            last_mouse_pos: 0,
            dragging_mouse: false,
            modified: false,
            active: true,
            x: self.x,
            y: self.y,
            width: self.width,
            visible_start: 0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Style {
    pub cursor_color: u32,
    pub text_color: TextColor,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            cursor_color: 0x99_99_99_FF,
            text_color: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Selection {
    start: i32,
    end: i32,
}

impl Selection {
    fn is_empty(&self) -> bool {
        self.start == self.end
    }

    fn sorted(&self) -> Self {
        if self.start < self.end {
            *self
        } else {
            Self {
                start: self.end,
                end: self.start,
            }
        }
    }

    fn clear_to(&mut self, idx: i32) {
        self.start = idx;
        self.end = idx;
    }
}

pub struct TextBox {
    buffer: Vec<char>,
    old_cursor_pos: i32,
    cursor_pos: i32,
    last_keypress: f32,
    valid_chars: HashSet<char>,

    selection: Selection,

    length_limit: usize,

    mouse_clicks: u32, // consecutive clicks in the same position within time limit
    last_mouse_click: f32,
    last_mouse_pos: i32,
    dragging_mouse: bool,

    modified: bool,
    active: bool,

    x: u16,
    y: u16,
    width: u16,
    visible_start: u16,
}

impl TextBox {
    pub fn set_pos(&mut self, (x, y): (u16, u16)) {
        self.x = x;
        self.y = y;
    }

    pub fn set_width(&mut self, width: u16, text_renderer: &TextRenderer) {
        self.width = width;
        let end = text_renderer
            .compute_width_chars(self.buffer[..self.cursor_pos as usize].iter().copied());
        let start = end.saturating_sub(self.width);
        self.visible_start = self.visible_start.max(start);
    }

    pub fn set_active(&mut self, active: bool, time_secs: f32, select: bool) {
        if self.active != active {
            self.active = active;
            self.last_keypress = time_secs;
            self.selection.clear_to(0);

            if active && select {
                self.select_all();
            }
        }
    }

    pub fn select_all(&mut self) {
        self.selection.start = 0;
        self.selection.end = self.buffer.len() as _;
        self.cursor_pos = self.selection.end;
    }

    pub fn contents(&self) -> &[char] {
        &self.buffer
    }

    pub fn modified(&self) -> bool {
        self.modified
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn selection(&self) -> &[char] {
        let sel = self.selection.sorted();
        &self.buffer[sel.start as usize..sel.end as usize]
    }

    pub fn set_contents(&mut self, text: &[char], text_renderer: &TextRenderer) {
        self.reset();
        self.buffer.extend_from_slice(&text);
        self.buffer.retain(|c| self.valid_chars.contains(c));
        self.cursor_pos = self.buffer.len() as i32;

        let end = text_renderer
            .compute_width_chars(self.buffer[..self.cursor_pos as usize].iter().copied());
        let start = end.saturating_sub(self.width);
        self.visible_start = self.visible_start.max(start);
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.selection.clear_to(0);
        self.cursor_pos = 0;
        self.modified = false;
        self.visible_start = 0;
    }
}

// Input handling
impl TextBox {
    pub fn process_event(&mut self, event: &WindowEvent, res: &mut Resources) -> bool {
        match event {
            &WindowEvent::ReceivedCharacter(char) => {
                self.process_char_input(char, res.input.keyboard_mods);
            }
            &WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(key),
                        ..
                    },
                ..
            } => {
                let mods = res.input.keyboard_mods;
                let ctrl = mods.ctrl();
                let shift = mods.shift();

                match key {
                    Key::C if ctrl => self.copy_text(&mut res.input.clipboard),
                    Key::V if ctrl => self.paste_text(&mut res.input.clipboard),
                    Key::X if ctrl => self.cut_text(&mut res.input.clipboard),
                    Key::A if ctrl => self.select_all(),

                    Key::Up if shift => self.select_range(0, self.cursor_pos),
                    Key::Down if shift => self.select_range(i32::MAX, self.cursor_pos),
                    Key::Up => self.clear_to(0),
                    Key::Down => self.clear_to(i32::MAX),

                    Key::Left if shift && ctrl => {
                        self.select_range(self.selection.start, self.find_left_delim_idx())
                    }
                    Key::Right if shift && ctrl => {
                        self.select_range(self.selection.start, self.find_right_delim_idx())
                    }

                    Key::Left if ctrl => self.clear_to(self.find_left_delim_idx()),
                    Key::Right if ctrl => self.clear_to(self.find_right_delim_idx()),

                    Key::Left if shift => {
                        self.select_range(self.selection.start, self.selection.end - 1)
                    }
                    Key::Right if shift => {
                        self.select_range(self.selection.start, self.selection.end + 1)
                    }

                    Key::Left if !self.selection.is_empty() => {
                        self.clear_to(self.selection.sorted().start)
                    }
                    Key::Right if !self.selection.is_empty() => {
                        self.clear_to(self.selection.sorted().end)
                    }

                    Key::Left => self.clear_to(self.cursor_pos - 1),
                    Key::Right => self.clear_to(self.cursor_pos + 1),

                    Key::D => self.clear_to(self.cursor_pos),
                    _ => return false,
                }
                self.last_keypress = res.time.secs_f32;
            }
            &WindowEvent::MouseInput {
                button: MouseButton::Left,
                state: ElementState::Pressed,
                ..
            } => {
                self.recompute_visible_start_if_needed(res.renderer.ui.text());

                let mouse_x = (res.input.mouse.pos().x + self.visible_start as f32).max(0.0) as u16;
                if mouse_x >= self.x {
                    let rel_x = mouse_x - self.x;
                    let pos = res
                        .renderer
                        .ui
                        .text()
                        .compute_glyph_idx_at_pos_chars(self.buffer.iter().copied(), rel_x)
                        as i32;

                    if pos != self.last_mouse_pos || res.time.secs_f32 - self.last_mouse_click > 0.3
                    {
                        self.mouse_clicks = 0;
                    }

                    self.cursor_pos = pos;
                    self.selection.clear_to(self.cursor_pos);
                    self.mouse_clicks += 1;
                    self.last_mouse_click = res.time.secs_f32;
                    self.last_mouse_pos = pos;
                    self.dragging_mouse = self.mouse_clicks == 1;

                    if self.mouse_clicks == 2 {
                        self.select_word();
                    } else if self.mouse_clicks == 3 {
                        self.select_all();
                    } else if self.mouse_clicks == 4 {
                        self.mouse_clicks = 1;
                    }
                }
            }
            &WindowEvent::MouseInput {
                button: MouseButton::Left,
                state: ElementState::Released,
                ..
            } => {
                self.dragging_mouse = false;
            }
            &WindowEvent::CursorMoved { .. } => {
                if self.dragging_mouse {
                    let mouse_x =
                        (res.input.mouse.pos().x + self.visible_start as f32).max(0.0) as u16;
                    let mouse_y =
                        res.window_size.extent.height as i32 - res.input.mouse.pos().y as i32;
                    if mouse_y - self.y as i32 > 40 {
                        self.selection.end = 0;
                        self.cursor_pos = 0;
                    } else if mouse_y - (self.y as i32) < -40 {
                        self.selection.end = self.buffer.len() as _;
                        self.cursor_pos = self.selection.end;
                    } else {
                        let rel_x = mouse_x.max(self.x) - self.x;
                        let pos = res
                            .renderer
                            .ui
                            .text()
                            .compute_glyph_idx_at_pos_chars(self.buffer.iter().copied(), rel_x)
                            as i32;
                        self.selection.end = pos;
                        self.cursor_pos = pos;
                    }
                }
            }
            _ => return false,
        }
        true
    }

    fn process_char_input(&mut self, c: char, mods: ModifiersState) {
        if c == BACKSPACE {
            if self.cursor_pos == 0 || !self.selection.is_empty() {
                self.erase_selection();
                return;
            }

            let mut idx = self.cursor_pos as usize - 1;
            if mods.ctrl() {
                idx = self.find_left_delim_idx() as usize;
            }

            self.select_range(self.cursor_pos, idx as i32);
            self.erase_selection();
            return;
        }

        if !self.valid_chars.contains(&c) {
            return;
        }

        if !self.selection.is_empty() {
            self.erase_selection();
        }

        if self.buffer.len() < self.length_limit {
            self.buffer.insert(self.cursor_pos as usize, c);
            self.clear_to(self.cursor_pos + 1);
            self.modified = true;
        }
    }

    fn clear_to(&mut self, cursor_idx: i32) {
        self.cursor_pos = cursor_idx.clamp(0, self.buffer.len() as _);
        self.selection.clear_to(self.cursor_pos);
    }

    fn select_range(&mut self, from: i32, to: i32) {
        self.selection.start = from.clamp(0, self.buffer.len() as _);
        self.selection.end = to.clamp(0, self.buffer.len() as _);
        self.cursor_pos = self.selection.end;
    }

    fn erase_selection(&mut self) {
        let sel = self.selection.sorted();
        if sel.is_empty() {
            return;
        }

        self.cursor_pos = sel.start;

        self.buffer.drain(sel.start as usize..sel.end as usize);
        self.selection.clear_to(sel.start);

        self.modified = true;
    }

    fn find_left_delim_idx(&self) -> i32 {
        if self.cursor_pos == 0 {
            return 0;
        }

        let mut idx = self.selection.end as usize - 1;
        while let Some(' ') = self.buffer.get(idx) {
            idx -= 1;
        }
        while idx > 0 && !CTRL_SEL_STOPPERS.contains(self.buffer[idx - 1]) {
            idx -= 1;
        }
        idx as i32
    }

    fn find_right_delim_idx(&self) -> i32 {
        if self.cursor_pos == self.buffer.len() as i32 {
            return self.buffer.len() as i32;
        }

        let mut idx = self.selection.end as usize;
        while let Some(' ') = self.buffer.get(idx) {
            idx += 1;
        }
        while idx < self.buffer.len() && !CTRL_SEL_STOPPERS.contains(self.buffer[idx]) {
            idx += 1;
        }
        idx as i32
    }

    fn paste_text(&mut self, clipboard: &mut Clipboard) {
        if let Ok(mut text) = clipboard.get_text() {
            text.retain(|c| self.valid_chars.contains(&c));
            let sel = self.selection.sorted();

            let length_limit =
                self.length_limit - self.buffer.len() + (sel.end - sel.start) as usize;
            let length = text.chars().count().min(length_limit);

            // Paste text
            self.buffer.splice(
                sel.start as usize..sel.end as usize,
                text.chars().take(length),
            );

            self.selection.clear_to(sel.start + length as i32);
            self.cursor_pos = self.selection.start;
            self.modified = true;
        }
    }

    fn copy_text(&mut self, clipboard: &mut Clipboard) {
        if !self.selection.is_empty() {
            let selected = self.selection().iter().collect();

            if let Err(e) = clipboard.set_text(selected) {
                println!("Error in writing to clipboard (ctrl c): {e}");
            }
        }
    }

    fn cut_text(&mut self, clipboard: &mut Clipboard) {
        if !self.selection.is_empty() {
            let sel = self.selection.sorted();
            let selected: String = self
                .buffer
                .drain(sel.start as usize..sel.end as usize)
                .collect();

            self.selection.clear_to(sel.start);
            self.cursor_pos = self.selection.start;
            self.modified = true;

            if let Err(e) = clipboard.set_text(selected) {
                println!("Error in writing to clipboard (ctrl x): {e}");
            }
        }
    }

    fn select_word(&mut self) {
        // Select word around cursor, delimited by whitespace

        let cursor_pos = (self.cursor_pos as usize).min(self.buffer.len() - 1);

        let mut start = cursor_pos.max(1) - 1;
        while start > 0 && !CTRL_SEL_STOPPERS.contains(self.buffer[start]) {
            start -= 1;
        }

        let mut end = cursor_pos;
        while end < self.buffer.len() && !CTRL_SEL_STOPPERS.contains(self.buffer[end]) {
            end += 1;
        }

        if CTRL_SEL_STOPPERS.contains(self.buffer[start]) {
            start += 1;
        }

        self.selection.start = start as _;
        self.selection.end = end as _;
        self.cursor_pos = end as _;
    }
}

// Rendering
impl TextBox {
    pub fn draw(&mut self, renderer: &mut UiRenderer, window_height: u16, time: f32) -> (u16, u16) {
        self.draw_styled(
            renderer,
            window_height,
            time,
            Style {
                cursor_color: 0xFF_FF_FF_FF,
                text_color: TextColor::default(),
            },
        )
    }

    pub fn draw_styled(
        &mut self,
        renderer: &mut UiRenderer,
        window_height: u16,
        time: f32,
        style: Style,
    ) -> (u16, u16) {
        self.recompute_visible_start_if_needed(renderer.text());

        let (x, y) = (self.x.wrapping_sub(self.visible_start), self.y);

        renderer.text().apply_scissors(
            (self.x, window_height - 30 - self.y + 5),
            (self.width as _, 30),
        );

        let sel = self.selection.sorted();
        let mut colors = [ColorRange::new(style.text_color, u32::MAX); 3];

        if !sel.is_empty() {
            colors[0] = ColorRange::new(style.text_color, sel.start as u32);
            colors[1] =
                ColorRange::from_rgba_n(0x11, 0x11, 0xFF, 0xFF, (sel.end - sel.start) as u32);
            colors[2] = ColorRange::new(style.text_color, u32::MAX);

            let sel_start_x = renderer
                .text()
                .compute_width_chars(self.buffer[..sel.start as usize].iter().copied());
            let sel_width = renderer.text().compute_width_chars(
                self.buffer[sel.start as usize..sel.end as usize]
                    .iter()
                    .copied(),
            );

            let x = (self.x + sel_start_x)
                .saturating_sub(self.visible_start)
                .clamp(self.x, self.x + self.width);
            let x2 = (self.x + sel_start_x + sel_width).saturating_sub(self.visible_start);

            let max_x = self.x + self.width;
            let width = (max_x - x).min(x2 - x);

            const SCALE: u16 = 3;
            renderer.draw_rect_xy_wh(
                (x.clamp(self.x, self.x + self.width), y - 2 * SCALE),
                (width, 10 * SCALE),
                0xA0_C7_F2_FF,
            );
        }

        let mut text_style = text_renderer::Style::default();
        text_style.colors = &colors;

        let cursor_x = renderer
            .text()
            .compute_width_chars(self.buffer[0..self.cursor_pos as usize].iter().copied());
        let (end_x, end_y) =
            renderer
                .text()
                .draw_2d_chars(self.buffer.iter().copied(), x, y, text_style);

        if sel.is_empty() && self.active && (time - self.last_keypress) % 1.0 < 0.5 {
            const SCALE: u16 = 3;
            renderer.draw_rect_xy_wh(
                (
                    (self.x + cursor_x - (SCALE - 1)).saturating_sub(self.visible_start),
                    y - 2 * SCALE,
                ),
                (2, 10 * SCALE),
                style.cursor_color,
            );
        }

        renderer.text().end_scissors();

        (end_x, end_y)
    }

    fn recompute_visible_start_if_needed(&mut self, text_renderer: &TextRenderer) {
        if self.old_cursor_pos != self.cursor_pos {
            let new_pos = self.cursor_pos;
            if new_pos < self.old_cursor_pos {
                let start = text_renderer
                    .compute_width_chars(self.buffer[..new_pos as usize].iter().copied());
                let rem_length = text_renderer
                    .compute_width_chars(self.buffer[new_pos as usize..].iter().copied());

                let full = start + rem_length;

                if self.visible_start + self.width > full {
                    self.visible_start = self
                        .visible_start
                        .min((start + rem_length).saturating_sub(self.width));
                } else {
                    self.visible_start = self.visible_start.min(start);
                }
            } else {
                let end = text_renderer
                    .compute_width_chars(self.buffer[..new_pos as usize].iter().copied());
                let start = end.saturating_sub(self.width);
                self.visible_start = self.visible_start.max(start);
            }
            self.old_cursor_pos = self.cursor_pos;
        }
    }
}

// Doesn't need to be called if no input events have been received
/* pub fn process_inputs(
    &mut self,
    keyboard: &mut Keyboard,
    mouse: &mut Mouse,
    clipboard: &mut Clipboard,
    text_renderer: &TextRenderer,
    window_height: u16,
    time: f32,
) {
    let old_cpos = self.cursor_pos;
    let old_sel = self.selection;
    self.handle_clipboard(keyboard, clipboard);
    self.update_cursor(keyboard, mouse, text_renderer, window_height, time);
    if self.selection != old_sel {
        self.last_keypress = time + 0.4;
    }

    let ctrl = keyboard.pressed(Key::LControl);
    let mut cpos = self.cursor_pos as usize;

    let mut modified = false;

    keyboard.drain_text_inputs().for_each(|c| {
        if self.valid_chars.contains(&c) {
            if !self.selection.is_empty() {
                self.erase_selection();
                cpos = self.cursor_pos as usize;
            }
            if self.buffer.len() < self.length_limit {
                self.buffer.insert(cpos, c);
                cpos += 1;
                modified = true;
            }
        } else {
            match c {
                '\x08' if !self.selection.is_empty() => {
                    modified = !self.selection.is_empty();
                    self.erase_selection();
                    cpos = self.cursor_pos as usize;
                }
                '\x08' if self.selection.is_empty() && cpos > 0 => {
                    let length_before = self.buffer.len();
                    // Backspace
                    if ctrl {
                        while let Some(' ') = self.buffer.get(cpos - 1) {
                            self.buffer.remove(cpos - 1);
                            cpos -= 1;

                            if cpos == 0 {
                                break;
                            }
                        }
                        while let Some(&char) = self.buffer.get(cpos - 1) {
                            if CTRL_SEL_STOPPERS.contains(char) {
                                break;
                            }
                            self.buffer.remove(cpos - 1);
                            cpos -= 1;

                            if cpos == 0 {
                                break;
                            }
                        }
                    } else {
                        self.buffer.remove(cpos - 1);
                        cpos -= 1;
                    }

                    if length_before != self.buffer.len() {
                        modified = true;
                    }
                }
                _ => {}
            }
        }
        self.last_keypress = time;
    });

    if modified {
        self.cursor_pos = cpos as _;
        self.selection.clear_to(self.cursor_pos);
        self.modified = true;
    }

    if old_cpos != self.cursor_pos {
        let new_pos = self.cursor_pos;
        if new_pos < old_cpos {
            let start = text_renderer
                .compute_width_chars(self.buffer[..new_pos as usize].iter().copied());
            let rem_length = text_renderer.compute_width_chars(self.buffer[new_pos as usize..].iter().copied());

            let full = start + rem_length;

            if self.visible_start + self.width > full {
                self.visible_start = self.visible_start.min((start + rem_length).saturating_sub(self.width));
            } else {
                self.visible_start = self.visible_start.min(start);
            }
        } else {
            let end = text_renderer
                .compute_width_chars(self.buffer[..new_pos as usize].iter().copied());
            let start = end.saturating_sub(self.width);
            self.visible_start = self.visible_start.max(start);
        }
    }
}

fn handle_clipboard(&mut self, keyboard: &mut Keyboard, clipboard: &mut Clipboard) {
    if keyboard.pressed_with_mods(Key::V, CTRL) {
        keyboard.release(Key::V);

        if let Ok(mut text) = clipboard.get_text() {
            text.retain(|c| self.valid_chars.contains(&c));
            let sel = self.selection.sorted();

            let length_limit = self.length_limit - self.buffer.len() + (sel.end - sel.start) as usize;
            let length = text.chars().count().min(length_limit);

            // Paste text
            self.buffer.splice(
                sel.start as usize..sel.end as usize,
                text.chars().take(length),
            );

            self.selection.clear_to(sel.start + length as i32);
            self.cursor_pos = self.selection.start;
            self.modified = true;
        }
    }

    if keyboard.pressed_with_mods(Key::C, CTRL) {
        keyboard.release(Key::C);

        if !self.selection.is_empty() {
            let selected = self.selection().iter().collect();

            if let Err(e) = clipboard.set_text(selected) {
                println!("Error in writing to clipboard (ctrl c): {e}");
            }
        }
    }

    if keyboard.pressed_with_mods(Key::X, CTRL) {
        keyboard.release(Key::X);

        if !self.selection.is_empty() {
            let sel = self.selection.sorted();
            let selected: String = self
                .buffer
                .drain(sel.start as usize..sel.end as usize)
                .collect();

            self.selection.clear_to(sel.start);
            self.cursor_pos = self.selection.start;
            self.modified = true;

            if let Err(e) = clipboard.set_text(selected) {
                println!("Error in writing to clipboard (ctrl x): {e}");
            }
        }
    }
} */

/* fn update_cursor(
    &mut self,
    keyboard: &mut Keyboard,
    mouse: &mut Mouse,
    renderer: &TextRenderer,
    win_height: u16,
    time_secs: f32,
) {
    if self.buffer.is_empty() {
        return; // prevent releasing left/right/up/down keys if buffer is empty
    }

    let mouse_x = (mouse.pos().x + self.visible_start as f32).max(0.0) as u16;
    if mouse_x >= self.x && mouse.just_pressed(MouseButton::Left) {
        let rel_x = mouse_x - self.x;
        let pos =
            renderer.compute_glyph_idx_at_pos_chars(self.buffer.iter().copied(), rel_x) as i32;

        if pos != self.last_mouse_pos || time_secs - self.last_mouse_click > 0.3 {
            self.mouse_clicks = 0;
        }

        self.cursor_pos = pos;
        self.selection.clear_to(self.cursor_pos);
        self.mouse_clicks += 1;
        self.last_mouse_click = time_secs;
        self.last_mouse_pos = pos;
        self.dragging_mouse = self.mouse_clicks == 1;

        if self.mouse_clicks == 2 {
            self.select_word();
        } else if self.mouse_clicks == 3 {
            self.select_all();
        } else if self.mouse_clicks == 4 {
            self.mouse_clicks = 1;
        }
    }

    if mouse.pressed(MouseButton::Left) && self.dragging_mouse {
        let mouse_y = win_height as i32 - mouse.pos().y as i32;
        if mouse_y - self.y as i32 > 40 {
            self.selection.end = 0;
            self.cursor_pos = 0;
        } else if mouse_y - (self.y as i32) < -40 {
            self.selection.end = self.buffer.len() as _;
            self.cursor_pos = self.selection.end;
        } else {
            let rel_x = mouse_x.max(self.x) - self.x;
            let pos =
                renderer.compute_glyph_idx_at_pos_chars(self.buffer.iter().copied(), rel_x) as i32;
            self.selection.end = pos;
            self.cursor_pos = pos;
        }
    }

    let mut dx = 0;
    if keyboard.release(Key::Right) {
        dx += 1;
    }
    if keyboard.release(Key::Left) {
        dx -= 1;
    }

    if keyboard.release(Key::Up) {
        dx = -self.cursor_pos;
        // Handled fine, just like for Left/Right as above. Makes it
        // possible to deselect with up/down even when above computation
        // would leave dx == 0.
        if self.cursor_pos == 0 {
            dx = -1;
        }
    }
    if keyboard.release(Key::Down) {
        dx = self.buffer.len() as i32 - self.cursor_pos;
        if self.cursor_pos == self.buffer.len() as i32 {
            dx = 1;
        }
    }

    if keyboard.pressed(Key::LControl) {
        if keyboard.release(Key::A) {
            self.selection.start = 0;
            self.selection.end = self.buffer.len() as i32;
            self.cursor_pos = self.selection.end;
            return;
        }

        if keyboard.release(Key::D) {
            self.selection.clear_to(self.cursor_pos);
            return;
        }

        if dx != 0 && (self.cursor_pos > 0 || dx > 0) {
            // Pls no hang, kthx
            let mut seek_pos = self.cursor_pos + dx;
            while let Some(&char) = self.buffer.get(seek_pos as usize) {
                if !CTRL_SEL_STOPPERS.contains(char) {
                    break;
                }
                seek_pos += dx;
            }
            while let Some(&char) = self.buffer.get(seek_pos as usize) {
                if CTRL_SEL_STOPPERS.contains(char) {
                    break;
                }
                seek_pos += dx;
            }
            if dx < 0 {
                seek_pos += 1;
            }
            dx = seek_pos as i32 - self.cursor_pos;
        }
    }

    if keyboard.pressed(Key::LShift) && dx != 0 {
        self.selection.end = (self.selection.end + dx).clamp(0, self.buffer.len() as i32);
        self.cursor_pos = self.selection.end;
    } else if !self.selection.is_empty() {
        if dx != 0 {
            let sel = self.selection.sorted();
            if keyboard.pressed(Key::LControl) {
                self.cursor_pos = (self.cursor_pos + dx).clamp(0, self.buffer.len() as i32);
            } else if dx > 0 {
                self.cursor_pos = sel.end;
            } else {
                self.cursor_pos = sel.start;
            }
            self.selection.clear_to(self.cursor_pos);
        }
    } else {
        let new_cursor_pos = (self.cursor_pos + dx).clamp(0, self.buffer.len() as i32);
        //if self.cursor_pos != new_cursor_pos {
            self.cursor_pos = new_cursor_pos;
            self.selection.clear_to(self.cursor_pos);
        //}
    }
} */

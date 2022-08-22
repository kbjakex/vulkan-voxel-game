use std::net::ToSocketAddrs;

use anyhow::bail;
use erupt::vk;
use flexstr::ToSharedStr;
use winit::{
    event::{ElementState, Event, MouseButton, WindowEvent, KeyboardInput},
    window::CursorIcon,
};

use crate::{
    game::{State, StateChange},
    input::{self, Key},
    networking::Connecting,
    renderer::{
        renderer::{Clear, OutdatedSwapchain, RendererState},
        text_renderer::{self, ColorRange, TextColor},
        ui_renderer::UiRenderer,
    },
    resources::Resources,
    text_box::{self, TextBox, TextBoxBuilder},
};

use super::game::GameState;

const ERR_COLOR: TextColor = TextColor::from_rgba(220, 50, 60, 255);

pub struct UsernameQueryState {
    username_box: TextBox,
    address_box: TextBox,

    connecting: Option<Connecting>,

    selected: u32,
    hovered: u32,

    message: String,
    message_color: TextColor,
}

impl State for UsernameQueryState {
    fn on_enter(&mut self, res: &mut crate::resources::Resources) -> anyhow::Result<()> {
        res.renderer
            .set_present_mode(vk::PresentModeKHR::FIFO_KHR)?; // strong vsync

        /* let text = res.renderer.ui.text();
        self.username_box
            .set_contents(&"jetp250".chars().collect::<Vec<char>>(), text, res.time.secs_f32);
        self.address_box
            .set_contents(&"localhost:29477".chars().collect::<Vec<char>>(), text, res.time.secs_f32);
        self.selected = 2; */

        Ok(())
    }

    fn on_update(
        &mut self,
        res: &mut crate::resources::Resources,
    ) -> Option<Box<crate::game::StateChange>> {
        let renderer = &mut res.renderer;
        let wsize = res.window_size.extent;
        let wsize = (wsize.width as u16, wsize.height as u16);

        let mouse_pos = res.input.mouse.pos();
        let hover = Self::get_hovering(
            wsize,
            (
                mouse_pos.x as u16,
                wsize.1.saturating_sub(mouse_pos.y as u16),
            ),
            self.connecting.is_some(),
        );

        if hover != self.hovered {
            self.hovered = hover;
            if hover != u32::MAX {
                if (hover == 0 || hover == 1) && self.connecting.is_none() {
                    res.window_handle.set_cursor_icon(CursorIcon::Text);
                } else {
                    res.window_handle.set_cursor_icon(CursorIcon::Hand);
                }
            } else {
                res.window_handle.set_cursor_icon(CursorIcon::Default);
            }
        }

        let kb = &mut res.input.keyboard;
        if self.connecting.is_some() {
            let anim_idx = (res.time.ms_u32 / 1000 % 4) as usize;
            self.message = "Connecting".to_owned() + &"...   "[3 - anim_idx..6 - anim_idx];

            let mut error = false;
            match self.connecting.as_mut().unwrap().try_tick_connection() {
                Ok(None) => {} // still connecting
                Ok(Some((response, connection))) => {
                    let username = self.username_box.contents().iter().collect();
                    let new_state = GameState::init(username, response, connection, res);

                    return Some(Box::new(StateChange::SwitchTo(Box::new(new_state))));
                }
                Err(err) => {
                    self.message = err.to_string();
                    self.message_color = ERR_COLOR;
                    error = true;
                }
            }

            if error || kb.release(Key::Return) || kb.release(Key::Space) {
                self.connecting = None;
                self.selected = 2; // back to join button
                if !error {
                    self.message.clear();
                }
            }
        } else {
            if kb.release(Key::Return) || (self.selected == 2 && kb.release(Key::Space)) {
                self.press_join_button();
            }

            if self.selected == 3 && kb.release(Key::Space) {
                return Some(Box::new(StateChange::Exit));
            }
        }

        self.draw_ui(&mut renderer.ui, wsize, self.hovered, res.time.secs_f32);

        if let Err(e) = self.render(res) {
            eprintln!("WARN: render() Err: {e}");
        }

        None
    }

    fn on_exit(&mut self, res: &mut crate::resources::Resources) -> anyhow::Result<()> {
        res.window_handle.set_cursor_icon(CursorIcon::Default);
        res.input.keyboard.clear_all();
        Ok(())
    }

    fn on_event(&mut self, event: &Event<()>, res: &mut Resources) -> Option<Box<StateChange>> {
        if input::handle_event(event, &mut res.input) {
            return None;
        }

        let Event::WindowEvent{ event, .. } = event else {
            return None;
        };

        match self.selected {
            0 => { self.username_box.process_event(event, res); },
            1 => { self.address_box.process_event(event, res); },
            _ => {}
        }

        match event {
            WindowEvent::KeyboardInput { 
                input: KeyboardInput{ virtual_keycode: Some(Key::Tab), state: ElementState::Pressed, .. }, .. 
            } if !res.input.keyboard_mods.alt() => {
                if res.input.keyboard_mods.shift() {
                    if self.selected == 0 {
                        self.selected = 3;
                    } else {
                        self.selected -= 1;
                    }
                } else {
                    if self.selected == 3 {
                        self.selected = 0;
                    } else {
                        self.selected += 1;
                    }
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if self.hovered != u32::MAX
                    && *state == ElementState::Pressed
                    && *button == MouseButton::Left
                {
                    if self.selected != self.hovered {
                        res.input.mouse.release(MouseButton::Left);
                    }
                    self.selected = self.hovered;

                    if self.connecting.is_some() {
                        if self.selected == 0 {
                            self.connecting = None;
                            self.selected = 2; // back to join button
                            self.message.clear();

                            let wsize = res.window_size.extent;
                            let wsize = (wsize.width as u16, wsize.height as u16);
                            let position = res.input.mouse.pos();

                            self.hovered = Self::get_hovering(
                                wsize,
                                (position.x as u16, wsize.1.saturating_sub(position.y as u16)),
                                self.connecting.is_some(),
                            );
                        }
                    } else {
                        if self.hovered == 2 {
                            self.press_join_button();
                        }
                        if self.hovered == 3 {
                            return Some(Box::new(StateChange::Exit));
                        }
                    }
                }
            }
            _ => {}
        }

        None
    }
}

impl UsernameQueryState {
    fn press_join_button(&mut self) {
        if self.connecting.is_some() {
            panic!("Bug: press_join_button() but self.connecting.is_some()");
        }

        self.hovered = 0;

        let username: String = self.username_box.contents().iter().collect();
        if username.len() < 3 {
            self.message = "Username is too short".to_owned();
            self.message_color = ERR_COLOR;
            return;
        }

        let address_str: String = self.address_box.contents().iter().collect();
        println!("Parsing '{address_str}'");
        let address = match address_str.trim().to_socket_addrs() {
            Ok(mut iter) => match iter.next() {
                Some(address) => address,
                None => {
                    self.message = format!("No such address");
                    self.message_color = ERR_COLOR;
                    return;
                }
            },
            Err(e) => {
                self.message = format!("Invalid address: {e}");
                self.message_color = ERR_COLOR;
                return;
            }
        };

        self.connecting = Some(Connecting::init_connection(
            address,
            username.to_shared_str(),
        ));
        self.message = "Connecting...".to_owned();
        self.message_color = TextColor::from_rgba32(0xa7a4bfFF);
    }

    fn draw_ui(&mut self, ui: &mut UiRenderer, win_size: (u16, u16), hover: u32, time_secs: f32) {
        let (w, h) = win_size;
        let (x1, y1) = (0, 0);
        let (x2, y2) = (w - 48, h - 48);

        const TEXT: TextColor = TextColor::from_rgba32(0xa7a4bfFF);
        const SELECTED: u32 = 0x4c4964FF;
        const UNSELECTED: u32 = 0x3c3a53FF;
        const HOVERED: u32 = 0x5d5b7aFF;

        let mut tbox_style = text_box::Style {
            cursor_color: 0xa7a4bfFF,
            text_color: TEXT,
        };

        // (Outline, fill)
        let mut colors = [(UNSELECTED, UNSELECTED); 4];
        colors[self.selected as usize] = (SELECTED, SELECTED);

        if hover != u32::MAX {
            colors[hover as usize] = (HOVERED, SELECTED);
        }

        let mut selected = self.selected;

        if self.connecting.is_some() {
            selected = u32::MAX;
            colors = [(UNSELECTED, 0x302F43FF); 4];
            tbox_style.text_color = TextColor::from_rgba32(0x4c4964FF);
        }

        // 4 corners
        ui.draw_rect_xy_wh((x1, y1), (48, 48), 0x4c4964FF);
        ui.draw_rect_xy_wh((x1 + 16, y1 + 16), (16, 16), 0x28263cFF);

        ui.draw_rect_xy_wh((x1, y2), (48, 48), 0x4c4964FF);
        ui.draw_rect_xy_wh((x1 + 16, y2 + 16), (16, 16), 0x28263cFF);

        ui.draw_rect_xy_wh((x2, y1), (48, 48), 0x4c4964FF);
        ui.draw_rect_xy_wh((x2 + 16, y1 + 16), (16, 16), 0x28263cFF);

        ui.draw_rect_xy_wh((x2, y2), (48, 48), 0x4c4964FF);
        ui.draw_rect_xy_wh((x2 + 16, y2 + 16), (16, 16), 0x28263cFF);

        // Edges
        ui.draw_rect_xy_wh((x1 + 64, y1), (x2 - x1 - 80, 32), 0x3c3a53FF);
        ui.draw_rect_xy_wh((x1 + 64, y2 + 16), (x2 - x1 - 80, 32), 0x3c3a53FF);
        ui.draw_rect_xy_wh((x1, y1 + 64), (32, y2 - y1 - 80), 0x3c3a53FF);
        ui.draw_rect_xy_wh((x2 + 16, y1 + 64), (32, y2 - y1 - 80), 0x3c3a53FF);

        ui.draw_rect_xy_wh((x1 + 80, y1), (x2 - x1 - 112, 16), 0x28263cFF);
        ui.draw_rect_xy_wh((x1 + 80, y2 + 32), (x2 - x1 - 112, 16), 0x28263cFF);
        ui.draw_rect_xy_wh((x1, y1 + 80), (16, y2 - y1 - 112), 0x28263cFF);
        ui.draw_rect_xy_wh((x2 + 32, y1 + 80), (16, y2 - y1 - 112), 0x28263cFF);

        // Text boxes
        ui.draw_text_colored("Username", w / 2 - 246 / 2 + 60, h / 2 + 60 + 63, TEXT);
        ui.draw_rect_xy_wh((w / 2 - 246 / 2, h / 2 + 60), (246, 53), colors[0].0);
        ui.draw_rect_xy_wh(
            (w / 2 - 246 / 2 + 2, h / 2 + 60 + 2),
            (246 - 4, 53 - 4),
            0x28263cFF,
        );
        ui.draw_rect_xy_wh(
            (w / 2 - 246 / 2 + 4, h / 2 + 60 + 4),
            (246 - 8, 53 - 8),
            colors[0].1,
        );
        self.username_box.set_active(selected == 0, time_secs, true);
        self.username_box
            .set_pos((w / 2 - 246 / 2 + 16, h / 2 + 60 + 17));
        self.username_box.draw_styled(ui, h, time_secs, tbox_style);

        ui.draw_text_colored(
            "Server address",
            w / 2 - 246 / 2 + 22,
            h / 2 - 41 + 63,
            TEXT,
        );
        ui.draw_rect_xy_wh((w / 2 - 246 / 2, h / 2 - 41), (246, 53), colors[1].0);
        ui.draw_rect_xy_wh(
            (w / 2 - 246 / 2 + 2, h / 2 - 41 + 2),
            (246 - 4, 53 - 4),
            0x28263cFF,
        );
        ui.draw_rect_xy_wh(
            (w / 2 - 246 / 2 + 4, h / 2 - 41 + 4),
            (246 - 8, 53 - 8),
            colors[1].1,
        );
        self.address_box.set_active(selected == 1, time_secs, true);
        self.address_box
            .set_pos((w / 2 - 246 / 2 + 16, h / 2 - 41 + 17));
        self.address_box.draw_styled(ui, h, time_secs, tbox_style);

        if self.connecting.is_some() {
            ui.draw_text_colored("Cancel", w / 2 - 78 / 2, h / 2 - 128 + 15, TEXT);
            ui.draw_rect_xy_wh((w / 2 - 112 / 2, h / 2 - 128), (112, 49), SELECTED);
            ui.draw_rect_xy_wh(
                (w / 2 - 112 / 2 + 2, h / 2 - 128 + 2),
                (112 - 4, 49 - 4),
                0x28263cFF,
            );
            ui.draw_rect_xy_wh(
                (w / 2 - 112 / 2 + 4, h / 2 - 128 + 4),
                (112 - 8, 49 - 8),
                SELECTED,
            );
        } else {
            // Join button
            ui.draw_text_colored("Join", w / 2 - 86 / 2 + 16 - 60, h / 2 - 128 + 15, TEXT);
            ui.draw_rect_xy_wh((w / 2 - 86 / 2 - 60, h / 2 - 128), (86, 49), colors[2].0);
            ui.draw_rect_xy_wh(
                (w / 2 - 86 / 2 + 2 - 60, h / 2 - 128 + 2),
                (86 - 4, 49 - 4),
                0x28263cFF,
            );
            ui.draw_rect_xy_wh(
                (w / 2 - 86 / 2 + 4 - 60, h / 2 - 128 + 4),
                (86 - 8, 49 - 8),
                colors[2].1,
            );

            ui.draw_text_colored("Quit", w / 2 - 86 / 2 + 16 + 60, h / 2 - 128 + 15, TEXT);
            ui.draw_rect_xy_wh((w / 2 - 86 / 2 + 60, h / 2 - 128), (86, 49), colors[3].0);
            ui.draw_rect_xy_wh(
                (w / 2 - 86 / 2 + 2 + 60, h / 2 - 128 + 2),
                (86 - 4, 49 - 4),
                0x28263cFF,
            );
            ui.draw_rect_xy_wh(
                (w / 2 - 86 / 2 + 4 + 60, h / 2 - 128 + 4),
                (86 - 8, 49 - 8),
                colors[3].1,
            );
        }

        if !self.message.is_empty() {
            let max_w = w - 60;
            let lines = ui.text().compute_linebreaks(&self.message, max_w);

            let mut prev = 0;
            let mut y = h / 2 - 162;
            for linebreak in lines {
                let line = &self.message[prev..linebreak as usize];
                let length = ui.text().compute_width(line);

                ui.text().draw_2d(
                    line,
                    w / 2 - length / 2,
                    y,
                    text_renderer::Style {
                        colors: &[ColorRange::new(self.message_color, u32::MAX)],
                        ..Default::default()
                    },
                );
                prev = linebreak as usize;
                if y < 30 {
                    break;
                }
                y -= 30;
            }
        }
    }

    fn get_hovering(win_size: (u16, u16), mouse_xy: (u16, u16), connecting: bool) -> u32 {
        let (w, h) = win_size;
        let (x, y) = mouse_xy;

        if connecting {
            if x >= w / 2 - 112 / 2
                && x <= w / 2 + 112 / 2
                && y >= h / 2 - 128
                && y <= h / 2 - 128 + 49
            {
                return 0; // Cancel button
            }
            return u32::MAX;
        }

        if x >= w / 2 - 246 / 2 && x <= w / 2 + 246 / 2 && y >= h / 2 + 60 && y <= h / 2 + 60 + 53 {
            return 0; // Username text box
        }

        if x >= w / 2 - 246 / 2 && x <= w / 2 + 246 / 2 && y >= h / 2 - 41 && y <= h / 2 - 41 + 53 {
            return 1; // Address box
        }

        if x >= w / 2 - 86 / 2 - 60
            && x <= w / 2 + 86 / 2 - 60
            && y >= h / 2 - 128
            && y <= h / 2 - 128 + 49
        {
            return 2; // Join button
        }

        if x >= w / 2 - 86 / 2 + 60
            && x <= w / 2 + 86 / 2 + 60
            && y >= h / 2 - 128
            && y <= h / 2 - 128 + 49
        {
            return 3; // Quit button
        }

        u32::MAX
    }
}

impl UsernameQueryState {
    fn render(&mut self, res: &mut Resources) -> anyhow::Result<()> {
        let renderer = &mut res.renderer;
        let ctx = match renderer.start_frame() {
            Ok(ctx) => ctx,
            Err(OutdatedSwapchain) => bail!("Outdated swapchain"),
        };

        if let Err(e) = UiRenderer::do_uploads(&mut renderer.ui, &mut renderer.vk, ctx.frame) {
            bail!("UiRenderer failed to upload vertices: {e}");
        };

        let vk = &renderer.vk;
        let RendererState {
            descriptors,
            render_passes,
            pipelines,
            framebuffers: _,
        } = &renderer.state;

        ctx.render_pass(
            &vk.device,
            &render_passes.ui.menu,
            ctx.swapchain_img_idx,
            Clear::Color(40.0 / 255.0, 38.0 / 255.0, 60.0 / 255.0),
            || {
                UiRenderer::render(
                    &mut renderer.ui,
                    &vk.device,
                    &ctx,
                    pipelines,
                    descriptors,
                    res.window_size.xy,
                );
            },
        );

        renderer.end_frame(ctx);
        Ok(())
    }
}

// Initialization
impl UsernameQueryState {
    pub fn new() -> anyhow::Result<Self> {
        let valid_username_chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
            .chars()
            .collect();

        let valid_address_chars =
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789:[]."
                .chars()
                .collect();
                
        Ok(Self {
            username_box: TextBoxBuilder::new_at(93, 317)
                .with_length_limit(14)
                .with_valid_chars(valid_username_chars)
                .with_width(246 - 2 * 16)
                .build(),
            address_box: TextBoxBuilder::new_at(93, 216)
                .with_length_limit(24)
                .with_valid_chars(valid_address_chars)
                .with_width(246 - 2 * 16)
                .build(),
            connecting: None,
            selected: 0,
            hovered: u32::MAX,
            message: String::new(),
            message_color: TextColor::default(),
        })
    }
}

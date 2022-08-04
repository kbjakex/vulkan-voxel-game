use anyhow::bail;
use erupt::vk;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, MouseButton, WindowEvent},
    window::CursorIcon,
};

use crate::{
    game::{State, StateChange},
    input::Key,
    renderer::{
        renderer::{Clear, OutdatedSwapchain, RendererState},
        text_renderer::TextColor,
        ui_renderer::UiRenderer,
    },
    resources::Resources,
};

use super::username_query::UsernameQueryState;

pub struct ConnectionLostState {
    hovered: bool,
}

impl State for ConnectionLostState {
    fn on_enter(&mut self, res: &mut crate::resources::Resources) -> anyhow::Result<()> {
        if !res.input.keyboard.is_in_text_input_mode() {
            res.input.keyboard.enter_text_input_mode();
        }

        res.renderer
            .set_present_mode(vk::PresentModeKHR::FIFO_KHR)?; // strong vsync

        let fullscreen_size = res.window_size.monitor_size_px;
        let window_size = LogicalSize::new(400, 480);

        res.window_handle.set_maximized(false);
        res.window_handle.set_inner_size(LogicalSize::new(400, 480));
        res.window_handle
            .set_outer_position(winit::dpi::LogicalPosition::new(
                fullscreen_size.width / 2 - window_size.width / 2,
                fullscreen_size.height / 2 - window_size.height / 2,
            ));

        Ok(())
    }

    fn on_update(
        &mut self,
        res: &mut crate::resources::Resources,
    ) -> Option<Box<crate::game::StateChange>> {
        let renderer = &mut res.renderer;
        let wsize = &res.window_size.extent;
        let wsize = (wsize.width as u16, wsize.height as u16);

        let kb = &mut res.input.keyboard;
        if kb.release(Key::Return) || kb.release(Key::Space) {
            return Some(Box::new(StateChange::SwitchTo(Box::new(
                UsernameQueryState::new().unwrap(),
            ))));
        }

        self.draw_ui(&mut renderer.ui, wsize, self.hovered);

        if let Err(e) = self.render(res) {
            eprintln!("WARN: render() Err: {e}");
        }

        None
    }

    fn on_exit(&mut self, res: &mut crate::resources::Resources) -> anyhow::Result<()> {
        res.input.keyboard.exit_text_input_mode();
        res.window_handle.set_cursor_icon(CursorIcon::Default);
        Ok(())
    }

    fn on_event(&mut self, event: &Event<()>, res: &mut Resources) -> Option<Box<StateChange>> {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                let wsize = res.window_size.extent;
                let wsize = (wsize.width as u16, wsize.height as u16);

                let hover = Self::get_hovering(
                    wsize,
                    (position.x as u16, wsize.1.saturating_sub(position.y as u16)),
                );

                if hover != self.hovered {
                    self.hovered = hover;
                    if hover {
                        res.window_handle.set_cursor_icon(CursorIcon::Hand);
                    } else {
                        res.window_handle.set_cursor_icon(CursorIcon::Default);
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button, .. },
                ..
            } => {
                if self.hovered && *state == ElementState::Pressed && *button == MouseButton::Left {
                    return Some(Box::new(StateChange::SwitchTo(Box::new(
                        UsernameQueryState::new().unwrap(),
                    ))));
                }
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                dbg![size, res.renderer.vk.swapchain.surface.extent];
            }
            _ => {}
        }
        None
    }
}

impl ConnectionLostState {
    fn draw_ui(&mut self, ui: &mut UiRenderer, win_size: (u16, u16), hover: bool) {
        let (w, h) = win_size;
        let (x1, y1) = (0, 0);
        let (x2, y2) = (w - 48, h - 48);

        const TEXT: TextColor = TextColor::from_rgba32(0xa7a4bfFF);
        const SELECTED: u32 = 0x4c4964FF;
        const HOVERED: u32 = 0x5d5b7aFF;

        // (Outline, fill)
        let mut colors = (SELECTED, SELECTED);
        if hover {
            colors = (HOVERED, SELECTED);
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

        ui.draw_text("Connection lost", w / 2 - 195 / 2, h / 2 + 30);

        // Join button
        ui.draw_text_colored("Ok", w / 2 - 33 / 2, h / 2 -45+15, TEXT);
        ui.draw_rect_xy_wh((w / 2 - 86 / 2, h / 2-45), (86, 49), colors.0);
        ui.draw_rect_xy_wh(
            (w / 2 - 86 / 2 + 2, h / 2 + 2-45),
            (86 - 4, 49 - 4),
            0x28263cFF,
        );
        ui.draw_rect_xy_wh(
            (w / 2 - 86 / 2 + 4, h / 2 + 4-45),
            (86 - 8, 49 - 8),
            colors.1,
        );
    }

    fn get_hovering(win_size: (u16, u16), mouse_xy: (u16, u16)) -> bool {
        let (w, h) = win_size;
        let (x, y) = mouse_xy;

        if x >= w / 2 - 86 / 2
            && x <= w / 2 + 86 / 2
            && y >= h / 2-45
            && y <= h / 2-45 + 49
        {
            return true; // Join button
        }

        false
    }
}

impl ConnectionLostState {
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
impl ConnectionLostState {
    pub fn new() -> Self {
        Self { hovered: false }
    }
}

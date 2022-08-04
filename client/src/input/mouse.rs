use glam::Vec2;
use winit::event::{MouseButton, MouseScrollDelta, ElementState, WindowEvent};

pub struct Mouse {
    pressed: Vec<u32>,
    just_released: Vec<(u32, u32)>,
    frame_counter: u32,

    moved: bool,

    pos: Vec2,
    prev_pos: Vec2, // pos last frame, not pos on previous update! Much more useful
    delta: Vec2,

    scroll_pos: f32,
    prev_scroll_pos: f32, // also pos last frame
}

impl Mouse {
    pub fn pressed(&self, button: MouseButton) -> bool {
        self.pressed_frames(button) > 0
    }

    pub fn pressed_frames(&self, button: MouseButton) -> u32 {
        self.pressed_frames_raw(mouse_button_to_index(button))
    }

    pub fn pressed_frames_raw(&self, button: usize) -> u32 {
        let timestamp = self.pressed[button as usize];
        if timestamp == 0 {
            0
        } else {
            self.frame_counter - timestamp
        }
    }

    pub fn just_pressed(&self, button: MouseButton) -> bool {
        self.pressed_frames(button) == 1
    }

    pub fn tapped(&self, button: MouseButton) -> bool {
        self.tapped_with_threshold(button, 7)
    }

    pub fn tapped_with_threshold(&self, button: MouseButton, max_frames: u32) -> bool {
        self.just_released_frames(button) <= max_frames
    }

    pub fn just_released(&self, button: MouseButton) -> bool {
        self.just_released_frames(button) > 0
    }

    pub fn just_released_frames(&self, button: MouseButton) -> u32 {
        let (frame_count, check) = self.just_released[mouse_button_to_index(button)];
        if check != self.frame_counter {
            0
        } else {
            frame_count
        }
    }

    pub fn release(&mut self, button: MouseButton) -> bool {
        self.release_get_frames(button) > 0
    }

    /// Gets the number of frames the button has been pressed, or 0 if it none
    pub fn release_get_frames(&mut self, button: MouseButton) -> u32 {
        let frames = self.pressed_frames(button);
        self.pressed[mouse_button_to_index(button)] = 0;
        frames
    }

    pub fn moved(&self) -> bool {
        self.moved
    }

    pub fn pos(&self) -> Vec2 {
        self.pos
    }

    pub fn prev_pos(&self) -> Vec2 {
        self.prev_pos
    }

    pub fn pos_delta(&self) -> Vec2 {
        self.delta
    }

    pub fn scroll_pos(&self) -> f32 {
        self.scroll_pos
    }

    pub fn prev_scroll_pos(&self) -> f32 {
        self.prev_scroll_pos
    }
}

pub struct MouseUpdater;

impl MouseUpdater {
    pub fn new_mouse(window_size: winit::dpi::LogicalSize<u32>) -> Mouse {
        let pos = Vec2::new(
            window_size.width as f32 / 2.0,
            window_size.height as f32 / 2.0,
        );

        let mut pressed = Vec::new();
        pressed.resize(32, 0); // ain't nobody got more than 32 buttons in a mouse

        let mut just_released = Vec::new();
        just_released.resize(32, (0, 0));

        Mouse {
            pressed,
            just_released,
            frame_counter: 0,
            moved: false,
            pos,
            prev_pos: pos,
            delta: Vec2::ZERO,
            scroll_pos: 0.0,
            prev_scroll_pos: 0.0,
        }
    }

    pub fn handle_mouse_events(event: &WindowEvent, mouse: &mut Mouse) -> bool {
        match event {
            WindowEvent::CursorMoved{position, ..} => {
                mouse.pos.x = position.x as f32;
                mouse.pos.y = position.y as f32;
            }
            WindowEvent::MouseWheel { delta, .. } => match delta {
                MouseScrollDelta::LineDelta(_, y) => {
                    mouse.scroll_pos += y;
                }
                MouseScrollDelta::PixelDelta(pos) => {
                    mouse.scroll_pos += pos.y as f32;
                }
            },
            WindowEvent::MouseInput { button, state, .. } => {
                let button = mouse_button_to_index(*button);
                match state {
                    ElementState::Pressed => {
                        if mouse.pressed[button] == 0 {
                            mouse.pressed[button] = mouse.frame_counter;
                        }
                    }
                    ElementState::Released => {
                        let frames_pressed = mouse.pressed_frames_raw(button);
                        mouse.pressed[button] = 0;
                        mouse.just_released[button] = (frames_pressed, mouse.frame_counter);
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// Called at the start of each frame
    pub fn first_tick(mouse: &mut Mouse) {
        // FP equality but fine because this should
        // only be the case if no mouse events occurred after
        // last_tick(), which sets these equal
        mouse.moved = mouse.pos != mouse.prev_pos;
        mouse.delta = mouse.pos - mouse.prev_pos;
    }

    /// Called at the end of each frame
    /// Purpose is to be able to keep track of the states
    /// before input events were received, which happens *before*
    /// first_tick() is called
    pub fn last_tick(mouse: &mut Mouse) {
        mouse.prev_pos = mouse.pos;
        mouse.prev_scroll_pos = mouse.scroll_pos;
        mouse.frame_counter += 1;
    }
}

fn mouse_button_to_index(button: MouseButton) -> usize {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
        MouseButton::Other(val) => val as usize,
    }
}
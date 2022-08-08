pub mod player;
pub mod states;

use std::time::Instant;

use erupt::vk;
use glam::{Vec2, Vec3};
use rayon::{prelude::IntoParallelIterator, ThreadPoolBuilder};
use winit::{
    dpi::{LogicalPosition, LogicalSize, PhysicalSize},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use crate::{
    camera::Camera,
    input::{self, KeyboardUpdater, MouseUpdater},
    renderer::renderer,
    resources::{
        core::{Time, WindowSize},
        metrics, Resources,
    },
};

use self::states::{connection_lost::ConnectionLostState, username_query::UsernameQueryState};

pub trait State {
    fn on_enter(&mut self, resources: &mut Resources) -> anyhow::Result<()>;
    fn on_update(&mut self, resources: &mut Resources) -> Option<Box<StateChange>>;
    fn on_exit(&mut self, resources: &mut Resources) -> anyhow::Result<()>;
    fn on_event(&mut self, event: &Event<()>, res: &mut Resources) -> Option<Box<StateChange>>;
}

pub enum StateChange {
    Exit, // calls on_exit() and pops the state off the stack
    SwitchTo(Box<dyn State>),
}

pub struct Game {
    pub resources: Box<Resources>,
    active_state: Box<dyn State>,
}

// Update logic
impl Game {
    // Called after all events have been processed
    pub fn update(&mut self, flow: &mut ControlFlow) {
        self.update_core_resources();

        if let Some(result) = self.active_state.on_update(&mut self.resources) {
            self.handle_state_change(result, flow);
        }

        // Update mouse again at end of tick
        MouseUpdater::last_tick(&mut self.resources.input.mouse);
    }

    fn update_core_resources(&mut self) {
        let prev_t = self.resources.time.secs_f32;

        let now = Instant::now();
        let time_res = &mut self.resources.time;
        time_res.now = now;
        time_res.secs_f32 = (now - time_res.at_launch).as_secs_f32();
        time_res.ms_u32 = (now - time_res.at_launch).as_millis() as u32;
        time_res.dt_secs = time_res.secs_f32 - prev_t;

        let timings = &mut self.resources.metrics.frame_time;
        let frametime = (now - timings.last_updated).as_secs_f32() * 1000.0;
        timings.frametime_history[self.resources.metrics.frame_count as usize & (timings.frametime_history.len()-1)] = frametime;

        let avg = timings.frametime_history.iter().sum::<f32>() / (timings.frametime_history.len() as f32);
        timings.avg_fps = 1000.0 / avg;
        timings.avg_frametime_ms = avg;
        timings.last_updated = now;

        self.resources.metrics.frame_count += 1;

        KeyboardUpdater::tick_keyboard(&mut self.resources.input.keyboard);
        MouseUpdater::first_tick(&mut self.resources.input.mouse);
    }
}

// Termination
impl Game {
    pub fn on_stop(&mut self) {
        println!("on_stop");
        if self.active_state.on_exit(&mut self.resources).is_err() {
            eprintln!("Error in state.on_exit()!");
        }

        self.resources.renderer.destroy_self();
    }
}

// Event handling
impl Game {
    pub fn on_event(&mut self, event: Event<()>, flow: &mut ControlFlow) {
        match &event {
            Event::MainEventsCleared => self.update(flow),
            Event::LoopDestroyed => self.on_stop(),
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *flow = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::Resized(PhysicalSize { width, height }),
                ..
            } => {
                let size = vk::Extent2D {
                    width: *width,
                    height: *height,
                };
                if self.resources.renderer.vk.swapchain.surface.extent == size {
                    println!("Prevented no-op window resize");
                    return;
                }

                println!("WindowEvent::Resized({}x{})", width, height);
                self.resources
                    .renderer
                    .handle_window_resize(*width, *height);

                let size = self.resources.renderer.vk.swapchain.surface.extent;
                self.resources.window_size = WindowSize {
                    extent: size,
                    xy: Vec2::new(size.width as f32, size.height as f32),
                    monitor_size_px: self.resources.window_size.monitor_size_px,
                };

                if let Some(result) = self.active_state.on_event(&event, &mut self.resources) {
                    self.handle_state_change(result, flow);
                }
            }
            Event::DeviceEvent { .. } | Event::WindowEvent { .. } => {
                let inputs = &mut self.resources.input;
                match &event {
                    Event::DeviceEvent { event, .. } => {
                        KeyboardUpdater::handle_key_event(event, &mut inputs.keyboard);
                    }
                    Event::WindowEvent { event, .. } => {
                        MouseUpdater::handle_mouse_events(event, &mut inputs.mouse);
                        KeyboardUpdater::handle_window_event(event, &mut inputs.keyboard);
                    }
                    _ => {}
                }

                if let Some(result) = self.active_state.on_event(&event, &mut self.resources) {
                    self.handle_state_change(result, flow);
                }
            }
            _ => {}
        }
    }
}

impl Game {
    fn handle_state_change(&mut self, change: Box<StateChange>, flow: &mut ControlFlow) {
        match *change {
            StateChange::Exit => *flow = ControlFlow::Exit,
            StateChange::SwitchTo(state) => {
                self.active_state.on_exit(&mut self.resources).unwrap();
                self.active_state = state;
                self.active_state.on_enter(&mut self.resources).unwrap();
            }
        }
    }
}

// Initialization
impl Game {
    pub fn init(event_loop: &EventLoop<()>) -> anyhow::Result<Self> {
        let fullscreen_size = event_loop.primary_monitor().unwrap().size();
        let fullscreen_size =
            fullscreen_size.to_logical(event_loop.primary_monitor().unwrap().scale_factor());

        let window_size = LogicalSize::new(400, 480);
        let window = WindowBuilder::new()
            .with_title("Game")
            .with_inner_size(window_size)
            .with_min_inner_size(LogicalSize::new(300, 450))
            .with_position(LogicalPosition::new(
                fullscreen_size.width / 2 - window_size.width / 2,
                fullscreen_size.height / 2 - window_size.height / 2,
            ))
            .build(&event_loop)
            .unwrap();

        let time = Instant::now();
        let default_camera = Camera::new(Vec3::ZERO, Vec2::new(400.0, 480.0));
        let renderer = renderer::init(&window, &default_camera)?;
        //window.set_inner_size(LogicalSize::new(512, 512));

        // Allocate all but one core/thread to the threadpool
        let thread_pool_threads = std::thread::available_parallelism()?.get() - 1;

        let mut resources = Box::new(Resources {
            time: Time {
                at_launch: time,
                now: time,
                ms_u32: 0,
                secs_f32: 0.0,
                dt_secs: 0.0,
            },
            window_handle: window,
            window_size: WindowSize {
                extent: erupt::vk::Extent2D {
                    width: window_size.width,
                    height: window_size.height,
                },
                xy: Vec2::new(window_size.width as f32, window_size.height as f32),
                monitor_size_px: fullscreen_size,
            },
            thread_pool: ThreadPoolBuilder::new()
                .num_threads(thread_pool_threads)
                .thread_name(|i| format!("Worker thread #{i}"))
                .build()?,
            metrics: metrics::Resources {
                frame_count: 0,
                frame_time: metrics::FrameTime {
                    avg_fps: 60.0, // whatever
                    avg_frametime_ms: 1000.0 / 60.0,
                    frametime_history: [1000.0 / 60.0; 32],
                    last_updated: time,
                },
            },
            renderer,
            input: input::init(window_size)?,
        });

        let mut active_state = Box::new(UsernameQueryState::new()?);
        active_state.on_enter(&mut resources)?;

        Ok(Self {
            resources,
            active_state,
        })
    }
}

pub mod input_recorder;

use std::{ffi::c_void, time::Instant, f32::consts::PI};

use erupt::vk::{self, BufferUsageFlags};
use flexstr::SharedStr;
use glam::{Mat4, Vec2, Vec3, DVec3, vec2, vec3, Quat, EulerRot};
use hecs::Entity;
use shared::{bits_and_bytes::{BitWriter, ByteReader, ByteWriter}, protocol::{s2c::login::LoginResponse, NetworkId, encode_angle_rad, encode_velocity, wrap_angle, decode_angle_rad}};
use smallvec::SmallVec;
use vkcore::{Buffer, BufferAllocation, UsageFlags, VkContext};
use winit::{event::{DeviceEvent, ElementState, Event, KeyboardInput, WindowEvent}, dpi::LogicalPosition, window::CursorGrabMode};

use crate::{
    camera::Camera,
    chat::{self, Chat},
    game::{State, StateChange, player::ThePlayer},
    input::Key,
    networking::Connection,
    renderer::{
        passes::terrain_pass::Vertex, renderer::Clear,
        ui_renderer::UiRenderer, wrappers::VertexBuffer,
    },
    resources::{
        core::{WindowSize, Time},
        game_state,
        Resources,
    }, world::{dimension::{ECS, Chunks}, chunk::WorldBlockPosExt, chunk_renderer::ChunkRenderer}, components::{Position, Velocity, HeadRotation},
};

use self::input_recorder::{InputRecorder, YawPitch};

use super::connection_lost::ConnectionLostState;

pub struct GameState {
    pub res: game_state::Resources,

    // Raw mouse motion; for camera only
    mouse_move_accumulator: Vec2,

    tick: u32,

    grid_vbo: VertexBuffer,
    cube_vbo: VertexBuffer,
}

impl State for GameState {
    fn on_enter(&mut self, res: &mut Resources) -> anyhow::Result<()> {
        let size = res
            .window_handle
            .primary_monitor()
            .unwrap()
            .size()
            .to_logical::<u32>(res.window_handle.scale_factor());
        println!("Window size: {size:?}");
        res.window_handle.set_inner_size(size);
        res.window_handle.set_maximized(true);
        res.window_handle.set_cursor_position(LogicalPosition::new(size.width / 2, size.height / 2))?;
        res.window_handle.set_cursor_grab(CursorGrabMode::Confined)?;
        res.window_handle.set_cursor_visible(false);
        println!("Entering GameState");

        self.grid_vbo = create_debug_grid(&mut res.renderer.vk)?;
        res.renderer
            .vk
            .uploader
            .flush_staged(&res.renderer.vk.device)?;

        dbg![res.input.keyboard.is_in_text_input_mode()];

        self.cube_vbo = create_debug_cube(&mut res.renderer.vk)?;

        Ok(())
    }

    fn on_update(&mut self, res: &mut Resources) -> Option<Box<StateChange>> {
        if let Some(st) = self.update_resources(res) {
            return Some(st);
        }

        if self.res.chunks.tick(res).is_err() {
            eprintln!("Error in Chunks::tick()");
            return Some(Box::new(StateChange::Exit));
        }

        // TODO...
        chat::process_text_input(
            &mut res.input.keyboard,
            &mut res.input.mouse,
            &mut self.res.chat,
            &mut res.renderer.ui,
            &mut self.res.net.connection,
            &mut res.input.clipboard,
            &res.window_size,
            &res.window_handle,
            res.time.secs_f32,
        ).unwrap(); // TODO

        res.renderer.ui.draw_text(&format!("FPS: {:.4}", res.metrics.frame_time.avg_fps), 30, res.window_size.extent.height as u16 - 60);
        res.renderer.ui.draw_text(&format!("X: {:.8}", self.res.camera.pos().x), 30, res.window_size.extent.height as u16 - 90);
        res.renderer.ui.draw_text(&format!("Y: {:.8}", self.res.camera.pos().y), 30, res.window_size.extent.height as u16 - 120);
        res.renderer.ui.draw_text(&format!("Z: {:.8}", self.res.camera.pos().z), 30, res.window_size.extent.height as u16 - 150);
        res.renderer.ui.draw_text(&format!("Yaw: {:.8}", self.res.camera.yaw().to_degrees()), 30, res.window_size.extent.height as u16 - 180);
        res.renderer.ui.draw_text(&format!("Pitch: {:.8}", self.res.camera.pitch().to_degrees()), 30, res.window_size.extent.height as u16 - 210);


        if let Err(e) = self.render(res) {
            eprintln!("render() error: {e}");
        }
        None
    }

    fn on_exit(&mut self, _res: &mut Resources) -> anyhow::Result<()> {
        println!("Exiting GameState");
        self.res.net.connection.send_disconnect();
        Ok(())
    }

    fn on_event(&mut self, event: &Event<()>, res: &mut Resources) -> Option<Box<StateChange>> {
        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                self.res.camera.on_window_resize(res.window_size.xy);
                self.res
                    .chat
                    .on_window_resize(res.window_size.extent.width as _, res.renderer.ui.text());
            }
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } => {
                if !self.res.chat.is_open() {
                    self.mouse_move_accumulator += vec2(delta.0 as f32, -delta.1 as f32);
                }
            }
            Event::WindowEvent { event: WindowEvent::Focused(focus_gained), .. } => {
                if !focus_gained && !self.res.chat.is_open() {
                    self.res.chat.toggle_open(&mut res.input.keyboard, &res.window_handle, res.window_size.xy).unwrap();
                }
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(Key::Escape),
                                ..
                            },
                        ..
                    },
                ..
            } => {
                return Some(Box::new(StateChange::Exit));
            }
            _ => {}
        }
        None
    }
}

// Networking
impl GameState {
    fn update_net(&mut self, res: &mut Resources) {
        let net = &mut self.res.net;

        net.connection.tick();

        if res.time.secs_f32 >= net.next_network_tick {
            net.network_tick_count += 1;
            net.next_network_tick = (net.network_tick_count as f64 * shared::TICK_DURATION.as_secs_f64()) as f32;
        }

        let net = &mut self.res.net;
        if let Some(channels) = net.connection.channels() {
            let ecs = &mut self.res.entities;

            while let Ok(mut bytes) = channels.entity_state_recv.try_recv() {
                let mut reader = ByteReader::new(&mut bytes);
                while reader.bytes_remaining() > 0 {
                    let id = NetworkId::from_raw(reader.read_u16());
                    let velocity = Vec3::new(
                        reader.read_f32(),
                        reader.read_f32(),
                        reader.read_f32()
                    );
                    let rotation = Vec2::new(
                        reader.read_f32(),
                        reader.read_f32()
                    );

                    if id == net.nid {
                        // bleh
                    } else {
                        if net.nid_to_entity_mapping.len() <= id.raw() as usize {
                            net.nid_to_entity_mapping.resize(id.raw() as usize + 1, (NetworkId::from_raw(0), Entity::DANGLING));
                        }

                        let (check_id, mut entity) = net.nid_to_entity_mapping[id.raw() as usize];
                        if check_id != id || entity == Entity::DANGLING {
                            if entity != Entity::DANGLING {
                                let _ = ecs.despawn(entity); // bad
                            }
                            println!("Spawning entity with id {id}");

                            // New entity then. TODO: add a proper way to spawn entities and make this an error
                            entity = ecs.spawn((id, Position(Vec3::ZERO), HeadRotation(Vec2::ZERO)));
                            net.nid_to_entity_mapping[id.raw() as usize] = (id, entity);

                            println!("Spawned entity with id {id:?}");
                        }

                        ecs.get::<&mut Position>(entity).unwrap().0 += velocity;
                        ecs.get::<&mut HeadRotation>(entity).unwrap().0 += rotation;

                        let p = ecs.get::<&mut Position>(entity).unwrap().0;
                        println!("Entity {id:?} is now at position {p}");
                    }
                }
            }
        }
    }
}

// Resources
impl GameState {
    fn update_resources(&mut self, res: &mut Resources) -> Option<Box<StateChange>> {
        self.update_camera(res);
        self.update_net(res);
        if self.res.net.connection.closed() {
            return Some(Box::new(StateChange::SwitchTo(Box::new(
                ConnectionLostState::new(),
            ))));
        }

        None
    }

    fn update_camera(&mut self, res: &mut Resources) {
        let keyboard = &mut res.input.keyboard;
        let camera = &mut self.res.camera;

        let right = keyboard.get_axis(Key::D, Key::A);
        let up = keyboard.get_axis(Key::Space, Key::LShift);
        let fwd = keyboard.get_axis(Key::W, Key::S);

        let mut velocity = self.res.the_player.vel * 0.9;
        
        if right != 0 || up != 0 || fwd != 0 {
            let (ys, yc) = camera.yaw().sin_cos();
            let fwd_dir = Vec3::new(yc, 0.0, ys);
            let up_dir = Vec3::Y;
            let right_dir = fwd_dir.cross(up_dir);

            let hor_acc = (right as f32 * right_dir + fwd as f32 * fwd_dir).normalize_or_zero();
            let acc = (hor_acc + up as f32 * up_dir) * 5.0;

            velocity = (velocity + acc).clamp_length_max(20.0);
        }
        self.res.the_player.vel = velocity;

        let mouse_speed = res.input.settings.mouse_sensitivity * 0.0025;
        let mouse_motion = self.mouse_move_accumulator * mouse_speed;
        self.mouse_move_accumulator = Vec2::ZERO;

        let mut nw_frames = SmallVec::new();
        let new_pos = self.res.input_recorder.integrator.step(velocity.as_dvec3() * res.time.dt_secs as f64, mouse_motion.as_dvec2(), res.time.dt_secs as f64, &mut nw_frames);
        camera.move_to(new_pos.0.0);
        camera.set_rotation(new_pos.1.0, new_pos.1.1);

        if !nw_frames.is_empty() && let Some(channels) = self.res.net.connection.channels() {
            for (Velocity(velocity), YawPitch(yaw, pitch)) in nw_frames {
                //println!("Moved {} units in NW tick", velocity.length());
                let mut payload = [0u8; 16];
    
                let mut writer = ByteWriter::new(&mut payload);
                let has_velocity = velocity.length_squared() != 0.0;
                let has_rotation = vec2(yaw, pitch).length_squared() != 0.0;

                if !has_velocity && !has_rotation {
                    continue;
                }

                writer.write_u32(self.tick);
                //println!("Delta for tick {}: {:.8}, {:.8}, {:.8}, pos {:.8}, {:.8}, {:.8}", self.tick, velocity.x, velocity.y, velocity.z, origin.x, origin.y, origin.z);
                self.tick += 1;
                writer.write_u8((has_velocity as u8) | ((has_rotation as u8) << 1));

                if has_velocity {
                    //println!("Moved {} units in nw frame", velocity.length());
                    writer.write_u16(encode_velocity(velocity.x) as u16);
                    writer.write_u16(encode_velocity(velocity.y) as u16);
                    writer.write_u16(encode_velocity(velocity.z) as u16);
                }
                if has_rotation {
                    //println!("Rot delta for tick {}: {:.8}, {:.8}, rot: {:.8}, {:.8}", self.tick, yaw.to_degrees(), pitch.to_degrees(), origin.x.to_degrees(), origin.y.to_degrees());
                    writer.write_u16(encode_angle_rad(shared::protocol::wrap_angle(yaw)) as u16);
                    writer.write_u16(encode_angle_rad(shared::protocol::wrap_angle(pitch)) as u16);
                }

                let len = writer.bytes_written();
    
                if let Err(e) = channels.player_state_send.send(payload[0..len].to_vec()) {
                    eprintln!("Error sending position data (channel closed?): {e}");
                }
            }
        }
        camera.update();
    }
}

impl GameState {
    fn draw_crosshair(ui: &mut UiRenderer, win_size: &WindowSize) {
        let (w, h) = (win_size.extent.width as u16, win_size.extent.height as u16);
        ui.draw_rect_xy_wh((w / 2 - 12, h / 2 - 1), (24, 2), 0x99_99_99_FF);
        ui.draw_rect_xy_wh((w / 2 - 1, h / 2 - 12), (2, 24), 0x99_99_99_FF);
    }

    fn render(&mut self, res: &mut Resources) -> anyhow::Result<()> {
        Self::draw_crosshair(&mut res.renderer.ui, &res.window_size);

        chat::draw_chat(
            &mut self.res.chat,
            res.time.secs_f32,
            &mut res.renderer.ui,
            &res.window_size,
        );

        let renderer = &mut res.renderer;
        let ctx = renderer.start_frame()?;

        let vk = &mut renderer.vk;
        let passes = &renderer.state.render_passes;

        UiRenderer::do_uploads(&mut renderer.ui, vk, ctx.frame)?;

        ctx.render_pass(
            &vk.device,
            &passes.terrain,
            0,
            Clear::ColorAndDepth([0.1, 0.1, 0.1], 0.0),
            || unsafe {
                vk.device.cmd_bind_pipeline(
                    ctx.commands,
                    vk::PipelineBindPoint::GRAPHICS,
                    renderer.state.pipelines.terrain.handle,
                );
                let pv = self.res.camera.proj_view_matrix();
                let pvm_ptr = &pv as *const Mat4 as *const c_void;
                vk.device.cmd_push_constants(
                    ctx.commands,
                    renderer.state.pipelines.terrain.layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    std::mem::size_of::<Mat4>() as u32,
                    pvm_ptr,
                );
                vk.device.cmd_bind_descriptor_sets(
                    ctx.commands,
                    vk::PipelineBindPoint::GRAPHICS,
                    renderer.state.pipelines.terrain.layout,
                    0,
                    &[renderer.state.descriptors.textures.descriptor_set],
                    &[],
                );
                vk.device.cmd_bind_vertex_buffers(
                    ctx.commands,
                    0,
                    &[self.grid_vbo.buffer.handle],
                    &[0],
                );
                vk.device
                    .cmd_draw(ctx.commands, self.grid_vbo.vertex_count, 1, 0, 0);

                vk.device.cmd_bind_vertex_buffers(
                    ctx.commands,
                    0,
                    &[self.cube_vbo.buffer.handle],
                    &[0],
                );

                self.res.entities.query::<(&Position, &HeadRotation)>().iter().for_each(|(_, (pos, rot))| {
                    let pv = self.res.camera.proj_view_matrix() * Mat4::from_translation(pos.0) * Mat4::from_euler(EulerRot::YXZ, rot.0.x + PI/2.0, -rot.0.y, 0.0);
                    let pvm_ptr = &pv as *const Mat4 as *const c_void;
                    vk.device.cmd_push_constants(
                        ctx.commands,
                        renderer.state.pipelines.terrain.layout,
                        vk::ShaderStageFlags::VERTEX,
                        0,
                        std::mem::size_of::<Mat4>() as u32,
                        pvm_ptr,
                    );
                    vk.device
                        .cmd_draw(ctx.commands, self.grid_vbo.vertex_count, 1, 0, 0);
                });
            },
        );

        ctx.render_pass(&vk.device, &passes.luma, 0, Clear::None, || unsafe {
            vk.device.cmd_bind_pipeline(
                ctx.commands,
                vk::PipelineBindPoint::GRAPHICS,
                renderer.state.pipelines.luma.handle,
            );
            vk.device.cmd_bind_descriptor_sets(
                ctx.commands,
                vk::PipelineBindPoint::GRAPHICS,
                renderer.state.pipelines.luma.layout,
                1,
                &[renderer.state.descriptors.attachments.luma_descriptor_set],
                &[],
            );

            vk.device.cmd_draw(ctx.commands, 3, 1, 0, 0);
        });
        ctx.render_pass(
            &vk.device,
            &passes.fxaa,
            ctx.swapchain_img_idx,
            Clear::Color(0.0, 0.0, 0.0),
            || unsafe {
                vk.device.cmd_bind_pipeline(
                    ctx.commands,
                    vk::PipelineBindPoint::GRAPHICS,
                    renderer.state.pipelines.fxaa.handle,
                );
                vk.device.cmd_bind_descriptor_sets(
                    ctx.commands,
                    vk::PipelineBindPoint::GRAPHICS,
                    renderer.state.pipelines.fxaa.layout,
                    1,
                    &[renderer.state.descriptors.attachments.fxaa_descriptor_set],
                    &[],
                );

                vk.device.cmd_draw(ctx.commands, 3, 1, 0, 0);
            },
        );
        ctx.render_pass(
            &vk.device,
            &passes.ui.game,
            ctx.swapchain_img_idx,
            Clear::None,
            || {
                UiRenderer::render(
                    &mut renderer.ui,
                    &vk.device,
                    &ctx,
                    &renderer.state.pipelines,
                    &renderer.state.descriptors,
                    res.window_size.xy,
                );
            },
        );

        renderer.end_frame(ctx);
        Ok(())
    }
}

// Initialization
impl GameState {
    pub fn init(
        username: SharedStr,
        login: LoginResponse,
        connection: Connection,
        res: &mut Resources,
    ) -> GameState {
        let time = Instant::now();
        res.time = Time {
            at_launch: time,
            now: time,
            ms_u32: 0,
            secs_f32: 0.0,
            dt_secs: 0.0,
        };

        Self {
            tick: 0,
            res: game_state::Resources {
                username,
                chat: Chat::new(res.window_size.extent.width as _),
                net: game_state::Net {
                    nid: login.network_id,
                    connection,
                    network_tick_count: 0,
                    next_network_tick: shared::TICK_DURATION.as_secs_f32(),
                    nid_to_entity_mapping: Vec::with_capacity(512)
                },
                camera: Camera::new(login.position, res.window_size.xy, f32::to_radians(80.0)),
                input_recorder: InputRecorder::new(login.position),
                entities: ECS::new(),
                chunks: Chunks::new(login.world_seed, 24, login.position.as_ivec3().to_chunk_pos()),
                the_player: ThePlayer::new(login.position),
                chunk_renderer: ChunkRenderer::new(),
            },
            mouse_move_accumulator: Vec2::ZERO,
            grid_vbo: VertexBuffer {
                buffer: Buffer::null(),
                vertex_count: 0,
            },
            cube_vbo: VertexBuffer { buffer: Buffer::null(), vertex_count: 0 }
        }
    }
}

fn create_debug_grid(vk: &mut VkContext) -> anyhow::Result<VertexBuffer> {
    let mut vertices: Vec<Vertex> = Vec::new();

    for x in -50..50 {
        for z in -50..50 {
            let (x, z) = (x as f32, z as f32);
            vertices.push(Vertex {
                pos: Vec3::new(x, 0.0, z),
                col: Vec3::ZERO,
                uv: (Vec2::new(x, z) / 100.0 + 0.5) * 100.0 / 16.0,
            });
            vertices.push(Vertex {
                pos: Vec3::new(x, 0.0, z + 1.0),
                col: Vec3::ZERO,
                uv: (Vec2::new(x, z + 1.0) / 100.0 + 0.5) * 100.0 / 16.0,
            });
            vertices.push(Vertex {
                pos: Vec3::new(x + 1.0, 0.0, z),
                col: Vec3::ZERO,
                uv: (Vec2::new(x + 1.0, z) / 100.0 + 0.5) * 100.0 / 16.0,
            });

            vertices.push(Vertex {
                pos: Vec3::new(x + 1.0, 0.0, z),
                col: Vec3::ZERO,
                uv: (Vec2::new(x + 1.0, z) / 100.0 + 0.5) * 100.0 / 16.0,
            });
            vertices.push(Vertex {
                pos: Vec3::new(x, 0.0, z + 1.0),
                col: Vec3::ZERO,
                uv: (Vec2::new(x, z + 1.0) / 100.0 + 0.5) * 100.0 / 16.0,
            });
            vertices.push(Vertex {
                pos: Vec3::new(x + 1.0, 0.0, z + 1.0),
                col: Vec3::ZERO,
                uv: (Vec2::new(x + 1.0, z + 1.0) / 100.0 + 0.5) * 100.0 / 16.0,
            });
        }
    }

    let mut buffer = vk.allocator.allocate_buffer(
        &vk.device,
        &BufferAllocation {
            size: vertices.len() * std::mem::size_of::<Vertex>(),
            usage: UsageFlags::FAST_DEVICE_ACCESS,
            vk_usage: BufferUsageFlags::VERTEX_BUFFER,
        },
    )?;

    vk.uploader
        .upload_to_buffer(&vk.device, &vertices[..], &mut buffer, 0)?;

    Ok(VertexBuffer {
        buffer,
        vertex_count: vertices.len() as u32,
    })
}

fn create_debug_cube(vk: &mut VkContext) -> anyhow::Result<VertexBuffer> {
    let mut vertices: Vec<Vertex> = Vec::new();

    let corners = [
        Vertex { pos: Vec3::new(-0.5, -0.5, -0.5), col: Vec3::ZERO, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(-0.5, -0.5, 0.5), col: Vec3::ZERO, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(-0.5, 0.5, -0.5), col: Vec3::ZERO, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(-0.5, 0.5, 0.5), col: Vec3::ZERO, uv: Vec2::ZERO },

        Vertex { pos: Vec3::new(0.5, -0.5, -0.5), col: Vec3::ZERO, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(0.5, -0.5, 0.5), col: Vec3::ZERO, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(0.5, 0.5, -0.5), col: Vec3::ZERO, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(0.5, 0.5, 0.5), col: Vec3::ZERO, uv: Vec2::ZERO },
    ];

    let indices = [
        [0, 1, 2], [2, 1, 3], // -X
        [4, 6, 5], [5, 6, 7], // +X
        [0, 2, 4], [4, 2, 6], // -Z
        [1, 5, 3], [3, 5, 7], // +Z
        [2, 3, 6], [6, 3, 7], // +Y
        [0, 4, 1], [1, 4, 5], // -Y
    ];

    for i in indices.iter().flatten().copied() {
        vertices.push(corners[i]);
    }

    let mut buffer = vk.allocator.allocate_buffer(
        &vk.device,
        &BufferAllocation {
            size: vertices.len() * std::mem::size_of::<Vertex>(),
            usage: UsageFlags::FAST_DEVICE_ACCESS,
            vk_usage: BufferUsageFlags::VERTEX_BUFFER,
        },
    )?;

    vk.uploader
        .upload_to_buffer(&vk.device, &vertices[..], &mut buffer, 0)?;

    Ok(VertexBuffer {
        buffer,
        vertex_count: vertices.len() as u32,
    })
}
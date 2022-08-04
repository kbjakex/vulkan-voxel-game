/* use std::ffi::c_void;

use erupt::vk;
use glam::{Mat4, Vec2, Vec3};
use vkcore::VkContext;
use winit::window::Window;

use crate::{
    camera::Camera, resources,
};

use super::{
    descriptor_sets::{DescriptorSets, SkyPushConstants},
    text_renderer::TextRendererUpdater,
    render_passes::RenderPasses, ui_renderer::UiRenderer,
};

pub const FRAMES_IN_FLIGHT: u32 = 2;

pub fn init(window: &Window) -> anyhow::Result<resources::renderer::Resources> {
    let mut vk = vkcore::VkContext::new(
        window,
        vkcore::VkConfig {
            /* present_mode: vk::PresentModeKHR::MAILBOX_KHR, */
            /* validation: vkcore::Validation::Disabled, */
            frames_in_flight: FRAMES_IN_FLIGHT,
            ..Default::default()
        },
    )
    .unwrap();

    let mut descriptors =
        DescriptorSets::create(&vk.device, &mut vk.uploader, &mut vk.allocator).unwrap();

    let render_passes = RenderPasses::init(&mut vk, &mut descriptors).unwrap();

    let camera = Camera::new(Vec3::ZERO, vk.swapchain.surface.extent);

    Ok(resources::renderer::Resources {
        ui: UiRenderer::create(&mut vk, &render_passes.ui_pass, &descriptors, &camera)?,
        vk
    })

    /* game
    .insert_resource(UiRenderer::create(&mut vk, &render_passes.ui_pass, &descriptors, &camera).unwrap())
    .insert_resource(camera)
    .insert_resource(descriptors)
    .insert_resource(RendererFrame {
        commands: vk::CommandBuffer::null(),
        swapchain_image_index: 0,
        frame_in_flight: 0,
    })
    .insert_resource(render_passes)
    .insert_resource(vk)
    .add_cleanup_system(renderer_cleanup)
    .add_resize_listener_system(recreate_renderer);

    register_systems() */
}

/* fn register_systems() -> SystemStage {
    let graph = SystemGraph::new();
    graph
        .root(begin_rendering_frame)
            .then(start_main_pass)
                .then(render_entities)
            .then(end_main_pass)
            .then(sky_pass)
            .then(luma_pass)
            .then(fxaa_pass)
            .then(begin_ui_pass)
                .then(render_ui)
            .then(end_ui_pass)
        .then(finish_rendering_frame);

    SystemStage::single_threaded().with_system_set(graph.into())
} */

/// Called when window is resized
fn recreate_renderer(
    mut passes: ResMut<RenderPasses>,
    mut descriptors: ResMut<DescriptorSets>,
    mut camera: ResMut<Camera>,
    mut vk: ResMut<vkcore::VkContext>,
    mut wsize: ResMut<WindowSize>,
    mut ui_renderer: ResMut<UiRenderer>,
) {
    let vk = &mut *vk;
    unsafe { vk.device.device_wait_idle() }.unwrap();

    vk.recreate_swapchain().unwrap();

    passes.on_window_resize(vk, &mut descriptors).unwrap();

    let extent = vk.swapchain.surface.extent;

    camera.on_window_resize(extent);

    wsize.extent = extent;
    wsize.xy = Vec2::new(extent.width as f32, extent.height as f32);

    ui_renderer.text().on_window_resize(
        vk,
        &passes.ui_pass.render_pass,
        &*descriptors,
    );
}

fn renderer_cleanup(
    mut passes: ResMut<RenderPasses>,
    mut descriptors: ResMut<DescriptorSets>,
    mut vk: ResMut<vkcore::VkContext>,
    mut ui_renderer: ResMut<UiRenderer>,
) {
    let vk = &mut *vk;
    unsafe { vk.device.device_wait_idle() }.result().unwrap();

    TextRendererUpdater::destroy(ui_renderer.text(), &vk.device, &mut vk.allocator).unwrap();

    descriptors
        .destroy_self(&vk.device, &mut vk.allocator)
        .unwrap();

    passes.destroy_self(&vk.device, &mut vk.allocator).unwrap();
    vk.destroy_self().unwrap();
}

pub struct RendererFrame {
    pub commands: vk::CommandBuffer,
    pub swapchain_image_index: u32,
    pub frame_in_flight: u32,
}

//
// FRAME SYNC
//

pub fn begin_rendering_frame(
    mut vk: ResMut<VkContext>,
    mut renderer_frame: ResMut<RendererFrame>,
    frames: Res<FrameCount>,
) {
    //println!("RENDERING START AT {}ms", (Instant::now() - time.now).as_secs_f32() * 1000.0);
    let vk = &mut *vk;
    renderer_frame.frame_in_flight = frames.0 % FRAMES_IN_FLIGHT;
    let frame_data = &vk.frames[renderer_frame.frame_in_flight as usize];
    renderer_frame.commands = frame_data.main_command_buffer;

    // Ensure any data transfers have finished
    vk.uploader.wait_fence_if_unfinished(&vk.device).unwrap();

    let device = &vk.device;

    unsafe {
        device
            .wait_for_fences(&[frame_data.render_fence], true, u64::MAX)
            .unwrap();

        device
            .reset_command_pool(frame_data.command_pool, vk::CommandPoolResetFlags::empty())
            .unwrap();
        device.reset_fences(&[frame_data.render_fence]).unwrap();
    }
    renderer_frame.swapchain_image_index = match vk
        .swapchain
        .image_idx_for_frame(&vk.frames[renderer_frame.frame_in_flight as usize], device)
    {
        Ok(idx) => idx,
        Err(_) => return, // swapchain needs to be recreated
    };

    let commands_begin_info = vk::CommandBufferBeginInfoBuilder::new()
        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

    unsafe { device.begin_command_buffer(renderer_frame.commands, &commands_begin_info) }.unwrap();
}

pub fn finish_rendering_frame(
    vk: Res<VkContext>,
    mut frames: ResMut<FrameCount>,
    renderer_frame: Res<RendererFrame>,
) {
    let frame = &vk.frames[renderer_frame.frame_in_flight as usize];
    let device = &vk.device;
    unsafe { vk.device.end_command_buffer(renderer_frame.commands) }.unwrap();

    unsafe {
        device.queue_submit(
            *device.queue,
            &[vk::SubmitInfoBuilder::new()
                .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                .wait_semaphores(&[frame.present_semaphore])
                .signal_semaphores(&[frame.render_semaphore])
                .command_buffers(&[renderer_frame.commands])],
            frame.render_fence,
        )
    }
    .unwrap();

    /*     println!("Presenting to {}", renderer_frame.swapchain_image_index);
     */
    unsafe {
        if let Err(e) = device
            .queue_present_khr(
                *device.queue,
                &vk::PresentInfoKHRBuilder::new()
                    .swapchains(&[vk.swapchain.handle])
                    .wait_semaphores(&[frame.render_semaphore])
                    .image_indices(&[renderer_frame.swapchain_image_index]),
            )
            .result()
        {
            println!("Check queue_present_khr! {}", e);
        }
    }
    frames.0 += 1; // Increment frame counter
}

//
// RENDERPASSES
//

pub fn start_main_pass(
    frame: Res<RendererFrame>,
    camera: Res<Camera>,
    passes: Res<RenderPasses>,
    descriptors: Res<DescriptorSets>,
    mut vk: ResMut<VkContext>
) {
    let vk = &mut *vk;
    let device = &vk.device;
    let frame = &*frame;
    let commands = frame.commands;

    let render_pass_info = vk::RenderPassBeginInfoBuilder::new()
        .clear_values(&[
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 0.0],
                },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 0.0,
                    stencil: 0,
                },
            },
        ])
        .render_pass(passes.terrain_pass.render_pass.handle)
        .render_area(vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: passes.terrain_pass.color_attachment.extent,
        })
        .framebuffer(passes.terrain_pass.render_pass.framebuffers[0]);

    unsafe {
        device.cmd_begin_render_pass(commands, &render_pass_info, vk::SubpassContents::INLINE);

        device.cmd_bind_pipeline(
            commands,
            vk::PipelineBindPoint::GRAPHICS,
            passes.terrain_pass.pipeline.handle,
        );
        let pv = camera.proj_view_matrix();
        let pvm_ptr = &pv as *const Mat4 as *const c_void;
        device.cmd_push_constants(
            commands,
            passes.terrain_pass.pipeline.layout,
            vk::ShaderStageFlags::VERTEX,
            0,
            std::mem::size_of::<Mat4>() as u32,
            pvm_ptr,
        );
        device.cmd_bind_descriptor_sets(
            commands,
            vk::PipelineBindPoint::GRAPHICS,
            passes.terrain_pass.pipeline.layout,
            0,
            &[descriptors.textures.descriptor_set],
            &[],
        );

        /*         device.cmd_bind_vertex_buffers(commands, 0, &[mesh.0.handle], &[0]);
        device.cmd_draw(commands, mesh.1 as _, 1, 0, 0); */

        
    }
}

fn render_entities(/* query: Query<(&Pos, &Rot, &Mesh)>,  */vk: Res<VkContext>, passes: Res<RenderPasses>, frame: Res<RendererFrame>, camera: Res<Camera>, descriptors: Res<DescriptorSets>) {
    let vk = &*vk;
    let pv = camera.proj_view_matrix();

    unsafe {
        vk.device.cmd_bind_pipeline(frame.commands, vk::PipelineBindPoint::GRAPHICS, passes.terrain_pass.pipeline.handle);
        vk.device.cmd_bind_descriptor_sets(
            frame.commands,
            vk::PipelineBindPoint::GRAPHICS,
            passes.terrain_pass.pipeline.layout,
            0,
            &[descriptors.textures.descriptor_set],
            &[],
        );
    }

    /* query.for_each(|(pos, rot, mesh)| {
        let pvm = pv * Mat4::from_translation(pos.0) * Mat4::from_rotation_x(rot.0.y) * Mat4::from_rotation_y(rot.0.x);
        unsafe {
            let pvm_ptr = &pvm as *const Mat4 as *const c_void;
            vk.device.cmd_push_constants(frame.commands, passes.terrain_pass.pipeline.layout, vk::ShaderStageFlags::VERTEX, 0, std::mem::size_of::<Mat4>() as u32, pvm_ptr);
            vk.device.cmd_bind_vertex_buffers(frame.commands, 0, &[mesh.0.handle], &[0]);

            vk.device.cmd_draw(frame.commands, 36, 1, 0, 0);
        }
    });   */
}

pub fn end_main_pass(
    frame: Res<RendererFrame>,
    vk: Res<VkContext>,
) {
    unsafe {
        vk.device.cmd_end_render_pass(frame.commands);
    }
}

pub fn sky_pass(
    camera: Res<Camera>,
    time: Res<Time>,
    frame: Res<RendererFrame>,
    passes: Res<RenderPasses>,
    descriptors: Res<DescriptorSets>,
    mut vk: ResMut<VkContext>,
) {
    let vk = &mut *vk;
    let device = &vk.device;
    let commands = frame.commands;
    let pass = &passes.sky_pass;

    let render_pass_info = vk::RenderPassBeginInfoBuilder::new()
        .clear_values(&[])
        .render_pass(pass.render_pass.handle)
        .render_area(vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: pass.color_attachment.extent,
        })
        .framebuffer(pass.render_pass.framebuffers[0]);

    unsafe {
        device.cmd_begin_render_pass(commands, &render_pass_info, vk::SubpassContents::INLINE);

        device.cmd_bind_pipeline(
            commands,
            vk::PipelineBindPoint::GRAPHICS,
            pass.pipeline.handle,
        );
        let sky_props = SkyPushConstants {
            inv_proj_view: camera.proj_view_matrix().inverse(),
            sun_azimuth: time.secs_f32.to_radians().cos() * 4.0,
        };
        let ipv_ptr = &sky_props as *const SkyPushConstants as *const c_void;
        device.cmd_push_constants(
            commands,
            pass.pipeline.layout,
            vk::ShaderStageFlags::FRAGMENT,
            0,
            std::mem::size_of::<SkyPushConstants>() as u32,
            ipv_ptr,
        );
        device.cmd_bind_descriptor_sets(
            commands,
            vk::PipelineBindPoint::GRAPHICS,
            pass.pipeline.layout,
            1,
            &[descriptors.attachments.sky_descriptor_set],
            &[],
        );

        device.cmd_draw(commands, 3, 1, 0, 0);

        device.cmd_end_render_pass(commands);
    }
}

pub fn luma_pass(
    frame: Res<RendererFrame>,
    passes: Res<RenderPasses>,
    descriptors: Res<DescriptorSets>,
    mut vk: ResMut<VkContext>,
) {
    let vk = &mut *vk;
    let device = &vk.device;
    let commands = frame.commands;
    let pass = &passes.luminance_pass;

    let render_pass_info = vk::RenderPassBeginInfoBuilder::new()
        .clear_values(&[])
        .render_pass(pass.render_pass.handle)
        .render_area(vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: pass.luminance_attachment.extent,
        })
        .framebuffer(pass.render_pass.framebuffers[0]);

    unsafe {
        device.cmd_begin_render_pass(commands, &render_pass_info, vk::SubpassContents::INLINE);

        device.cmd_bind_pipeline(
            commands,
            vk::PipelineBindPoint::GRAPHICS,
            pass.pipeline.handle,
        );
        device.cmd_bind_descriptor_sets(
            commands,
            vk::PipelineBindPoint::GRAPHICS,
            pass.pipeline.layout,
            1,
            &[descriptors.attachments.luma_descriptor_set],
            &[],
        );

        device.cmd_draw(commands, 3, 1, 0, 0);

        device.cmd_end_render_pass(commands);
    }
}

pub fn fxaa_pass(
    frame: Res<RendererFrame>,
    passes: Res<RenderPasses>,
    descriptors: Res<DescriptorSets>,
    mut vk: ResMut<VkContext>,
) {
    let vk = &mut *vk;
    let device = &vk.device;
    let commands = frame.commands;
    let pass = &passes.fxaa_pass;

    let render_pass_info = vk::RenderPassBeginInfoBuilder::new()
        .clear_values(&[])
        .render_pass(pass.render_pass.handle)
        .render_area(vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk.swapchain.surface.extent,
        })
        .framebuffer(pass.render_pass.framebuffers[frame.swapchain_image_index as usize]);

    unsafe {
        device.cmd_begin_render_pass(commands, &render_pass_info, vk::SubpassContents::INLINE);

        device.cmd_bind_pipeline(
            commands,
            vk::PipelineBindPoint::GRAPHICS,
            pass.pipeline.handle,
        );
        device.cmd_bind_descriptor_sets(
            commands,
            vk::PipelineBindPoint::GRAPHICS,
            pass.pipeline.layout,
            1,
            &[descriptors.attachments.fxaa_descriptor_set],
            &[],
        );

        device.cmd_draw(commands, 3, 1, 0, 0);

        device.cmd_end_render_pass(commands);
    }
}

pub fn begin_ui_pass(
    frame: Res<RendererFrame>,
    passes: Res<RenderPasses>,
    mut vk: ResMut<VkContext>,
) {
    let vk = &mut *vk;
    let device = &vk.device;
    let commands = frame.commands;
    let pass = &passes.ui_pass;

    let render_pass_info = vk::RenderPassBeginInfoBuilder::new()
        .clear_values(&[])
        .render_pass(pass.render_pass.handle)
        .render_area(vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk.swapchain.surface.extent,
        })
        .framebuffer(pass.render_pass.framebuffers[frame.swapchain_image_index as usize]);

    unsafe {
        device.cmd_begin_render_pass(commands, &render_pass_info, vk::SubpassContents::INLINE);
    }
}

fn render_ui(
    mut vk: ResMut<VkContext>, 
    mut ui_renderer: ResMut<UiRenderer>,
    passes: Res<RenderPasses>, 
    frame: Res<RendererFrame>,
    descriptors: Res<DescriptorSets>,
    wnd_size: Res<WindowSize>
) {
    let vk = &mut *vk;
    unsafe {
        vk.device.cmd_bind_pipeline(frame.commands, vk::PipelineBindPoint::GRAPHICS, passes.ui_pass.pipeline.handle);
        // `2.0 / ..` because coordinate space is from -1 to 1 (so 2 units)
        let pv = 2.0 / wnd_size.xy;
        let pvm_ptr = &pv as *const Vec2 as *const c_void;
        vk.device.cmd_push_constants(
            frame.commands,
            passes.ui_pass.pipeline.layout,
            vk::ShaderStageFlags::VERTEX,
            0,
            std::mem::size_of::<Vec2>() as u32,
            pvm_ptr,
        );

        let n_verts = ui_renderer.upload_vertices(vk).unwrap();
        vk.device.cmd_bind_vertex_buffers(frame.commands, 0, &[ui_renderer.buffer.handle], &[0]);
        vk.device.cmd_draw(frame.commands, n_verts, 1, 0, 0);

        TextRendererUpdater::render_all(
            ui_renderer.text(),
            vk,
            *wnd_size,
            &descriptors,
            frame.commands,
            frame.frame_in_flight as usize,
        )
        .unwrap();
    }
}

pub fn end_ui_pass(
    frame: Res<RendererFrame>,
    vk: Res<VkContext>
) {
    unsafe {
        vk.device.cmd_end_render_pass(frame.commands);
    }
} */
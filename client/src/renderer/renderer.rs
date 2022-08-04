use std::fmt::Display;

use erupt::vk;
use smallvec::SmallVec;
use vkcore::{Device, RenderPass, Validation, VkContext};
use winit::window::Window;

use crate::camera::Camera;

use super::{
    descriptor_sets::DescriptorSets, framebuffers::FramebufferImages, pipelines::Pipelines,
    render_passes::RenderPasses, ui_renderer::UiRenderer,
};

pub const FRAMES_IN_FLIGHT: u32 = 2;
pub const VALIDATION: Validation = Validation::EnabledWithDefaults;
pub const PRESENT_MODE: vk::PresentModeKHR = vk::PresentModeKHR::FIFO_KHR;

pub struct RendererState {
    pub descriptors: DescriptorSets,
    pub render_passes: RenderPasses,
    pub pipelines: Pipelines,
    pub framebuffers: FramebufferImages,
}

pub enum Clear {
    None,
    Color(f32, f32, f32),
    ColorAndDepth([f32; 3], f32),
}

pub struct RenderContext {
    pub frame: usize,
    pub swapchain_img_idx: usize,
    pub commands: vk::CommandBuffer,
}

impl RenderContext {
    pub fn render_pass<F>(
        &self,
        device: &Device,
        pass: &RenderPass,
        framebuffer_idx: usize,
        clear: Clear,
        callback: F,
    ) where
        F: FnOnce(),
    {
        let mut clear_values: SmallVec<[vk::ClearValue; 2]> = SmallVec::new();
        match clear {
            Clear::None => {}
            Clear::Color(r, g, b) => {
                clear_values.push(vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [r, g, b, 1.0],
                    },
                });
            }
            Clear::ColorAndDepth(rgb, depth) => {
                clear_values.push(vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [rgb[0], rgb[1], rgb[2], 1.0],
                    },
                });
                clear_values.push(vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue { depth, stencil: 0 },
                })
            }
        }
        let clear_values = &clear_values[..];

        let render_pass_info = vk::RenderPassBeginInfoBuilder::new()
            .clear_values(clear_values)
            .render_pass(pass.handle)
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: pass.extent,
            })
            .framebuffer(pass.framebuffers[framebuffer_idx]);

        unsafe {
            device.cmd_begin_render_pass(
                self.commands,
                &render_pass_info,
                vk::SubpassContents::INLINE,
            );
        }

        callback();

        unsafe {
            device.cmd_end_render_pass(self.commands);
        }
    }
}

pub struct Renderer {
    pub vk: VkContext,
    pub ui: UiRenderer,
    pub state: RendererState,
    frame: usize,
}

#[derive(Debug)]
pub struct OutdatedSwapchain;

impl std::error::Error for OutdatedSwapchain {}

impl Display for OutdatedSwapchain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Swapchain is outdated and needs to be recreated.")
    }
}

impl Renderer {
    pub fn start_frame(&mut self) -> Result<RenderContext, OutdatedSwapchain> {
        let vk = &mut self.vk;
        let frame_in_flight = (self.frame as u32 % FRAMES_IN_FLIGHT) as usize;
        let frame_data = &mut vk.frames[frame_in_flight as usize];
        let command_buffer = frame_data.main_command_buffer;

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
        let swapchain_image_index = match vk.swapchain.image_idx_for_frame(frame_data, device) {
            Ok(idx) => idx,
            Err(_) => return Err(OutdatedSwapchain), // swapchain needs to be recreated
        };

        let commands_begin_info = vk::CommandBufferBeginInfoBuilder::new()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe { device.begin_command_buffer(command_buffer, &commands_begin_info) }.unwrap();

        Ok(RenderContext {
            frame: frame_in_flight,
            swapchain_img_idx: swapchain_image_index as usize,
            commands: command_buffer,
        })
    }

    pub fn end_frame(&mut self, ctx: RenderContext) {
        let vk = &mut self.vk;
        let frame_data = &mut vk.frames[ctx.frame];
        let device = &vk.device;

        unsafe { vk.device.end_command_buffer(ctx.commands) }.unwrap();

        unsafe {
            device.queue_submit(
                *device.queue,
                &[vk::SubmitInfoBuilder::new()
                    .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                    .wait_semaphores(&[frame_data.present_semaphore])
                    .signal_semaphores(&[frame_data.render_semaphore])
                    .command_buffers(&[ctx.commands])],
                frame_data.render_fence,
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
                        .wait_semaphores(&[frame_data.render_semaphore])
                        .image_indices(&[ctx.swapchain_img_idx as _]),
                )
                .result()
            {
                println!("Check queue_present_khr! {}", e);
            }
        }
        self.frame += 1; // Increment frame counter
    }

    pub fn set_present_mode(&mut self, present_mode: vk::PresentModeKHR) -> anyhow::Result<()> {
        let vk = &mut self.vk;
        if vk.present_mode == present_mode {
            return Ok(());
        }

        vk.present_mode = present_mode; // ??? todo investigate what's this
        vk.swapchain.present_mode = present_mode;
        unsafe { vk.device.device_wait_idle() }.unwrap(); // Fails if device lost or OOM
        vk.recreate_swapchain()?;

        self.state.render_passes.handle_window_resize(
            vk,
            &mut self.state.descriptors,
            &self.state.framebuffers,
        )?;

        // No need to recreate pipelines because viewport size didn't change, and framebuffer images
        // because they would be recreated identical

        Ok(())
    }
}

impl Renderer {
    pub fn handle_window_resize(&mut self, width: u32, height: u32) {
        let vk = &mut self.vk;
        vk.swapchain.surface.extent = vk::Extent2D { width, height };
        unsafe { vk.device.device_wait_idle() }.unwrap(); // Fails if device lost or OOM
        vk.recreate_swapchain().unwrap(); // Safe, should never fail here

        self.state.framebuffers.handle_window_resize(vk).unwrap(); // TODO unwrap()
        self.state
            .render_passes
            .handle_window_resize(vk, &mut self.state.descriptors, &self.state.framebuffers)
            .unwrap(); // TODO unwrap()

        self.state.pipelines.destroy_self(&vk.device);
        self.state.pipelines =
            Pipelines::init(vk, &self.state.render_passes, &self.state.descriptors).unwrap();
        // TODO unwrap()

        UiRenderer::handle_window_resize(&mut self.ui, vk);
    }
}

impl Renderer {
    pub fn destroy_self(&mut self) {
        unsafe {
            self.vk.device.device_wait_idle().unwrap();
        }

        if let Err(e) = self.ui.destroy_self(&mut self.vk) {
            eprintln!("Error destroying UI renderer: {e}");
        }

        self.state.pipelines.destroy_self(&self.vk.device);
        self.state.render_passes.destroy_self(&self.vk.device);

        if let Err(e) = self
            .state
            .framebuffers
            .destroy_self(&self.vk.device, &mut self.vk.allocator)
        {
            eprintln!("Error destroying framebuffers: '{e}'");
        }

        if let Err(e) = self
            .state
            .descriptors
            .destroy_self(&self.vk.device, &mut self.vk.allocator)
        {
            eprintln!("Error destroying descriptor sets: '{e}'");
        }

        if let Err(e) = self.vk.destroy_self() {
            eprintln!("Error in vulkan de-initialization: '{e}'");
        }
    }
}

pub fn init(window: &Window, camera: &Camera) -> anyhow::Result<Renderer> {
    let mut vk = vkcore::VkContext::new(
        window,
        vkcore::VkConfig {
            present_mode: PRESENT_MODE,
            validation: VALIDATION,
            frames_in_flight: FRAMES_IN_FLIGHT,
            ..Default::default()
        },
    )?;

    let mut descriptors = DescriptorSets::create(&mut vk)?;
    let framebuffers = FramebufferImages::init(&mut vk)?;
    let render_passes = RenderPasses::init(&mut vk, &mut descriptors, &framebuffers)?;
    let pipelines = Pipelines::init(&mut vk, &render_passes, &descriptors)?;

    let ui = UiRenderer::create(&mut vk, &descriptors, camera)?;

    Ok(Renderer {
        vk,
        ui,
        state: RendererState {
            descriptors,
            framebuffers,
            pipelines,
            render_passes,
        },
        frame: 0,
    })
}

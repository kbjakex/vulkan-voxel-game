use crate::Device;
use erupt::vk;
use smallvec::SmallVec;

pub struct FrameData {
    pub present_semaphore: vk::Semaphore,
    pub render_semaphore: vk::Semaphore,
    pub render_fence: vk::Fence,

    pub command_pool: vk::CommandPool,
    pub main_command_buffer: vk::CommandBuffer,
}

impl FrameData {
    pub fn destroy_self(&self, device: &Device) {
        unsafe {
            device.destroy_semaphore(self.present_semaphore, None);
            device.destroy_semaphore(self.render_semaphore, None);
            device.destroy_fence(self.render_fence, None);

            device.destroy_command_pool(self.command_pool, None);
        }
    }
}

pub struct RenderPass {
    pub handle: vk::RenderPass,
    pub framebuffers: SmallVec<[vk::Framebuffer; 2]>,
    pub extent: vk::Extent2D,
}

impl RenderPass {
    pub fn null() -> Self {
        Self {
            handle: vk::RenderPass::null(),
            framebuffers: Default::default(),
            extent: Default::default(),
        }
    }

    pub fn recreate_framebuffers(
        &mut self,
        device: &Device,
        img: FramebufferImages,
        depth_attachment: Option<vk::ImageView>,
    ) {
        for fb in self.framebuffers.iter().copied() {
            if !fb.is_null() {
                unsafe {
                    device.destroy_framebuffer(fb, None);
                }
            }
        }

        let extent = vk::Extent2D {
            width: img.width,
            height: img.height,
        };

        if self.extent != extent {
            eprintln!("WARN: RenderPass::recreate_framebuffers() changed extent from {:?} to {:?}!", self.extent, extent);
            self.extent = extent;
        }

        self.framebuffers = img
            .views
            .iter()
            .map(|&view| {
                let mut attachments = SmallVec::<[vk::ImageView; 2]>::new();
                attachments.push(view);
                if let Some(depth_texture) = depth_attachment {
                    attachments.push(depth_texture);
                }

                let framebuffer_info = vk::FramebufferCreateInfoBuilder::new()
                    .render_pass(self.handle)
                    .attachments(&attachments)
                    .width(img.width)
                    .height(img.height)
                    .layers(1);

                unsafe { device.create_framebuffer(&framebuffer_info, None) }.unwrap()
            })
            .collect();
    }

    pub fn destroy_self(&self, device: &Device) {
        unsafe {
            for &fbo in &self.framebuffers {
                device.destroy_framebuffer(fbo, None);
            }

            device.destroy_render_pass(self.handle, None);
        }
    }
}

pub use vk::AttachmentLoadOp as LoadOp;
pub use vk::AttachmentStoreOp as StoreOp;

pub struct SubpassDesc<'a> {
    pub color_attachment_refs: &'a [AttachmentRef],
    pub input_attachment_refs: &'a [AttachmentRef],
    pub depth_attachment_ref: Option<AttachmentRef>,
    pub pipeline_bind_point: vk::PipelineBindPoint,
}

pub struct AttachmentRef {
    pub attachment_idx: u32,
    pub layout: vk::ImageLayout,
}

pub struct ColorAttachment {
    pub format: vk::Format,
    pub load_op: LoadOp,
    pub store_op: StoreOp,
    pub initial_layout: vk::ImageLayout,
    pub final_layout: vk::ImageLayout,
}

#[derive(Copy, Clone)]
pub struct DepthAttachment {
    pub view: vk::ImageView,
    pub format: vk::Format,

    pub load_op: LoadOp,
    pub store_op: StoreOp,
    pub initial_layout: vk::ImageLayout,
    pub final_layout: vk::ImageLayout,
}

/* impl Default for DepthAttachment {
    fn default() -> Self {
        Self {
            view: vk::ImageView::null(),
            format: vk::Format::D32_SFLOAT,
            samples: vk::SampleCountFlagBits::_1,
            load_op: LoadOp::CLEAR,
            store_op: StoreOp::STORE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        }
    }
} */

pub struct SubpassDependency {
    pub src_subpass: u32,
    pub dst_subpass: u32,
    pub src_stage_mask: vk::PipelineStageFlags,
    pub dst_stage_mask: vk::PipelineStageFlags,
    pub src_access_mask: vk::AccessFlags,
    pub dst_access_mask: vk::AccessFlags,
    pub dependency_flags: vk::DependencyFlags,
}

pub struct FramebufferImages<'a> {
    pub width: u32,
    pub height: u32,
    pub views: &'a [vk::ImageView],
}

pub struct RenderPassDescriptor<'a> {
    pub color_attachments: &'a [ColorAttachment],
    pub depth_attachment: Option<DepthAttachment>,
    pub subpasses: &'a [SubpassDesc<'a>],
    pub dependencies: &'a [SubpassDependency],
    pub framebuffer_images: FramebufferImages<'a>,
}

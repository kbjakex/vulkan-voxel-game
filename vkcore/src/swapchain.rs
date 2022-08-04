use erupt::{vk, DeviceLoader};
use smallvec::SmallVec;

use crate::{
    render_pass::{RenderPass, RenderPassDescriptor},
    Device, FrameData,
};

use anyhow::{Context, Result};

pub struct Surface {
    pub handle: vk::SurfaceKHR,
    pub format: vk::SurfaceFormatKHR,
    pub extent: vk::Extent2D,
}

impl Surface {
    pub fn aspect_ratio(&self) -> f32 {
        self.extent.width as f32 / self.extent.height as f32
    }
}

pub struct Swapchain {
    pub handle: vk::SwapchainKHR,
    pub surface: Surface,
    pub present_mode: vk::PresentModeKHR,

    pub images: SmallVec<[vk::Image; 2]>,
    pub image_views: SmallVec<[vk::ImageView; 2]>,
}

impl Swapchain {
    pub fn image_idx_for_frame(&self, frame: &FrameData, device: &Device) -> Result<u32> {
        let idx = unsafe {
            device.acquire_next_image_khr(
                self.handle,
                u64::MAX,
                frame.present_semaphore,
                vk::Fence::null(),
            )
        }.result()?;
        Ok(idx)
    }

    pub fn create_render_pass(
        &self,
        device: &Device,
        desc: RenderPassDescriptor,
    ) -> Result<RenderPass> {
        let depth_texture = desc.depth_attachment.map(|attachment| attachment.view);
   
        let mut pass = RenderPass {
            handle: self.make_vk_render_pass(&device.logical, &desc)?,
            framebuffers: SmallVec::new(),
            extent: vk::Extent2D {
                width: desc.framebuffer_images.width,
                height: desc.framebuffer_images.height,
            }
        };

        pass.recreate_framebuffers(device, desc.framebuffer_images, depth_texture);

        Ok(pass)
    }

    fn make_vk_render_pass(
        &self,
        gpu: &DeviceLoader,
        desc: &RenderPassDescriptor,
    ) -> Result<vk::RenderPass> {
        let mut color_attachment_refs: SmallVec<[vk::AttachmentReferenceBuilder; 4]> =
            SmallVec::new();
        let mut input_attachment_refs: SmallVec<[vk::AttachmentReferenceBuilder; 4]> =
            SmallVec::new();
        let mut depth_attachment_refs: SmallVec<[vk::AttachmentReference; 4]> = SmallVec::new();

        for subpass in desc.subpasses {
            for color_ref in subpass.color_attachment_refs {
                color_attachment_refs.push(
                    vk::AttachmentReferenceBuilder::new()
                        .attachment(color_ref.attachment_idx)
                        .layout(color_ref.layout),
                );
            }

            for input_ref in subpass.input_attachment_refs {
                input_attachment_refs.push(
                    vk::AttachmentReferenceBuilder::new()
                        .attachment(input_ref.attachment_idx)
                        .layout(input_ref.layout),
                );
            }

            if let Some(depth) = &subpass.depth_attachment_ref {
                depth_attachment_refs.push(
                    *vk::AttachmentReferenceBuilder::new()
                        .attachment(depth.attachment_idx)
                        .layout(depth.layout),
                )
            }
        }

        let mut subpasses = Vec::new();
        let mut color_ref_idx = 0;
        let mut input_ref_idx = 0;
        let mut depth_ref_idx = 0;
        for subpass in desc.subpasses {
            let color_end = color_ref_idx + subpass.color_attachment_refs.len();
            let input_end = input_ref_idx + subpass.input_attachment_refs.len();

            let input_attachments = if input_attachment_refs.is_empty() {
                &[]
            } else {
                &input_attachment_refs[input_ref_idx..input_end]
            };

            let mut pass = vk::SubpassDescriptionBuilder::new()
                .pipeline_bind_point(subpass.pipeline_bind_point)
                .color_attachments(&color_attachment_refs[color_ref_idx..color_end])
                .input_attachments(input_attachments);

            if subpass.depth_attachment_ref.is_some() {
                pass = pass.depth_stencil_attachment(&depth_attachment_refs[depth_ref_idx]);
                depth_ref_idx += 1;
            }

            subpasses.push(pass);

            color_ref_idx = color_end;
            input_ref_idx = input_end;
        }

        let mut attachment_descs: SmallVec<[vk::AttachmentDescriptionBuilder; 3]> = SmallVec::new();

        for attachment in desc.color_attachments {
            attachment_descs.push(
                vk::AttachmentDescriptionBuilder::new()
                    .format(attachment.format)
                    .samples(vk::SampleCountFlagBits::_1)
                    .initial_layout(attachment.initial_layout)
                    .final_layout(attachment.final_layout)
                    .load_op(attachment.load_op)
                    .store_op(attachment.store_op)
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE),
            );
        }
        if let Some(depth) = desc.depth_attachment {
            attachment_descs.push(
                vk::AttachmentDescriptionBuilder::new()
                    .format(depth.format)
                    .samples(vk::SampleCountFlagBits::_1)
                    .load_op(depth.load_op)
                    .store_op(depth.store_op)
                    .initial_layout(depth.initial_layout)
                    .final_layout(depth.final_layout)
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE),
            );
        }

        let mut dependencies: SmallVec<[vk::SubpassDependencyBuilder; 4]> = SmallVec::new();
        for dep in desc.dependencies {
            dependencies.push(
                vk::SubpassDependencyBuilder::new()
                    .src_subpass(dep.src_subpass)
                    .dst_subpass(dep.dst_subpass)
                    .src_stage_mask(dep.src_stage_mask)
                    .dst_stage_mask(dep.dst_stage_mask)
                    .src_access_mask(dep.src_access_mask)
                    .dst_access_mask(dep.dst_access_mask)
                    .dependency_flags(dep.dependency_flags),
            );
        }

        let render_pass_info = vk::RenderPassCreateInfoBuilder::new()
            .attachments(&attachment_descs)
            .subpasses(&subpasses)
            .dependencies(&dependencies);

        unsafe { gpu.create_render_pass(&render_pass_info, None) }
            .map_err(|e| e)
            .context("create_render_pass")
    }

    pub(crate) unsafe fn destroy_self(&mut self, device: &Device) {
        for &view in &self.image_views {
            device.destroy_image_view(view, None);
        }

        device.destroy_swapchain_khr(self.handle, None);
    }
}

use vkcore::{Device, RenderPass, VkContext};

use erupt::vk;
use glam::Vec2;

use anyhow::Result;

use crate::renderer::descriptor_sets::TexelSizeUBO;

use super::{
    descriptor_sets::DescriptorSets, framebuffers::FramebufferImages,
    passes::ui_pass::UiRenderPasses,
};

/*
Each render pass may have one or more pipelines.
One renderpass draws to screen, and therefore requires more than one framebuffer. Rest are fine with one.
*/

pub struct RenderPasses {
    pub terrain: RenderPass,
    /* pub sky: RenderPass, */
    pub luma: RenderPass,
    pub fxaa: RenderPass,
    pub ui: UiRenderPasses,
}

impl RenderPasses {
    pub fn init(
        vk: &mut VkContext,
        descriptors: &mut DescriptorSets,
        fbs: &FramebufferImages,
    ) -> Result<RenderPasses> {
        use super::passes::*;

        let result = RenderPasses {
            terrain: terrain_pass::create_render_pass(vk, fbs)?,
            /* sky: sky_pass::create_render_pass(vk, fbs)?, */
            luma: luminance_pass::create_render_pass(vk, fbs)?,
            fxaa: fxaa_pass::create_render_pass(vk)?,
            ui: ui_pass::create_render_pass(vk)?,
        };

        result.update_descriptors_and_uniforms(vk, descriptors, fbs)?;

        Ok(result)
    }

    fn update_descriptors_and_uniforms(
        &self,
        vk: &mut VkContext,
        descriptors: &mut DescriptorSets,
        fbs: &FramebufferImages,
    ) -> Result<()> {
        let wsize = vk.swapchain.surface.extent;
        vk.uploader.upload_to_buffer(
            &vk.device,
            &[TexelSizeUBO {
                texel_size: Vec2::new(1.0 / wsize.width as f32, 1.0 / wsize.height as f32),
            }],
            &mut descriptors.attachments.fxaa_ubo_buf,
            0,
        )?;
        vk.uploader.flush_staged(&vk.device)?;

        unsafe {
            vk.device.update_descriptor_sets(
                &[
                    vk::WriteDescriptorSetBuilder::new()
                        .dst_binding(0)
                        .dst_set(descriptors.attachments.fxaa_descriptor_set)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(&[vk::DescriptorImageInfoBuilder::new()
                            .image_view(fbs.main_pass_color.view)
                            .sampler(descriptors.attachments.sampler)
                            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]),
                    vk::WriteDescriptorSetBuilder::new()
                        .dst_binding(1)
                        .dst_set(descriptors.attachments.fxaa_descriptor_set)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(&[vk::DescriptorImageInfoBuilder::new()
                            .image_view(fbs.luma.view)
                            .sampler(descriptors.attachments.sampler)
                            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]),
                    vk::WriteDescriptorSetBuilder::new()
                        .dst_binding(2)
                        .dst_set(descriptors.attachments.fxaa_descriptor_set)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .buffer_info(&[vk::DescriptorBufferInfoBuilder::new()
                            .range(vk::WHOLE_SIZE)
                            .buffer(descriptors.attachments.fxaa_ubo_buf.handle)
                            .offset(0)]),
                    /*                     vk::WriteDescriptorSetBuilder::new()
                        .dst_binding(0)
                        .dst_set(descriptors.attachments.sky_descriptor_set)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(&[vk::DescriptorImageInfoBuilder::new()
                            .image_view(fbs.main_pass_color.view)
                            .sampler(descriptors.attachments.sampler)
                            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]),
                    vk::WriteDescriptorSetBuilder::new()
                        .dst_binding(1)
                        .dst_set(descriptors.attachments.sky_descriptor_set)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(&[vk::DescriptorImageInfoBuilder::new()
                            .image_view(fbs.depth.view)
                            .sampler(descriptors.attachments.sampler)
                            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]), */
                    vk::WriteDescriptorSetBuilder::new()
                        .dst_binding(0)
                        .dst_set(descriptors.attachments.luma_descriptor_set)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(&[vk::DescriptorImageInfoBuilder::new()
                            .image_view(fbs.main_pass_color.view)
                            .sampler(descriptors.attachments.sampler)
                            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]),
                ],
                &[],
            );
        }
        Ok(())
    }

    // Update FrameBuffers first!
    pub fn handle_window_resize(
        &mut self,
        vk: &mut VkContext,
        descriptors: &mut DescriptorSets,
        fbs: &FramebufferImages,
    ) -> Result<()> {
        use super::passes::*;

        terrain_pass::handle_window_resize(&mut self.terrain, vk, fbs);
        /* sky_pass::handle_window_resize(&mut self.luma, vk, fbs); */
        luminance_pass::handle_window_resize(&mut self.luma, vk, fbs);    
        fxaa_pass::handle_window_resize(&mut self.fxaa, vk);
        ui_pass::handle_window_resize(&mut self.ui.game, vk);
        ui_pass::handle_window_resize(&mut self.ui.menu, vk);

        self.update_descriptors_and_uniforms(vk, descriptors, fbs)
    }

    pub fn destroy_self(&mut self, device: &Device) {
        self.terrain.destroy_self(device);
        /* self.sky.destroy_self(device); */
        self.luma.destroy_self(device);
        self.fxaa.destroy_self(device);
        self.ui.game.destroy_self(device);
        self.ui.menu.destroy_self(device);
    }
}

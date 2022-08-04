use erupt::vk;
use vkcore::{RenderPass, pipeline::Pipeline, VkContext};

use crate::{assets, renderer::{descriptor_sets::DescriptorSets, framebuffers::FramebufferImages}};

use anyhow::Result;

pub fn create_render_pass(vk: &VkContext, fbs: &FramebufferImages) -> Result<RenderPass> {
    vk.create_render_pass(vkcore::RenderPassDescriptor {
        color_attachments: &[vkcore::ColorAttachment {
            format: fbs.luma.format,
            load_op: vkcore::LoadOp::DONT_CARE,
            store_op: vkcore::StoreOp::STORE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }],
        depth_attachment: None,
        subpasses: &[vkcore::SubpassDesc {
            color_attachment_refs: &[vkcore::AttachmentRef {
                attachment_idx: 0,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            }],
            input_attachment_refs: &[],
            depth_attachment_ref: None,
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
        }],
        dependencies: &[
            vkcore::SubpassDependency {
                src_subpass: vk::SUBPASS_EXTERNAL,
                dst_subpass: 0, // first and last subpass
                src_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
                src_access_mask: vk::AccessFlags::SHADER_READ,
                dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                dependency_flags: vk::DependencyFlags::BY_REGION,
            },
            vkcore::SubpassDependency {
                src_subpass: 0,
                dst_subpass: vk::SUBPASS_EXTERNAL,
                src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
                dst_access_mask: vk::AccessFlags::SHADER_READ,
                dependency_flags: vk::DependencyFlags::BY_REGION,
            }
        ],
        framebuffer_images: vkcore::FramebufferImages {
            width: fbs.luma.extent.width,
            height: fbs.luma.extent.height,
            views: &[fbs.luma.view],
        },
    })
}

pub fn create_pipelines(render_pass: &RenderPass, vk: &VkContext, descriptors: &DescriptorSets) -> Result<Pipeline> {
    let extent = vk.swapchain.surface.extent;
    use vk::ColorComponentFlags as CCF;
    vk
        .graphics_pipeline_builder()
        .render_pass(render_pass)
        .vertex_code(assets::postprocess_pipelines::FULLSCREEN_SHADER_VERT)
        .fragment_code(assets::postprocess_pipelines::LUMA_SHADER_FRAG)
        .rasterization_state(
            vk::PipelineRasterizationStateCreateInfoBuilder::new()
                .cull_mode(vk::CullModeFlags::NONE)
                .line_width(1.0)
                .polygon_mode(vk::PolygonMode::FILL)
                .depth_bias_enable(false)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .rasterizer_discard_enable(false),
        )
        .input_info(
            vk::PipelineVertexInputStateCreateInfoBuilder::new()
                .vertex_binding_descriptions(&[])
                .vertex_attribute_descriptions(&[]),
        )
        .blend_attachment(
            vk::PipelineColorBlendAttachmentStateBuilder::new()
                .blend_enable(false)
                .color_write_mask(CCF::R | CCF::G | CCF::B | CCF::A),
        )
        .layout(
            vk::PipelineLayoutCreateInfoBuilder::new()
                .set_layouts(&[descriptors.textures.layout, descriptors.attachments.luma_layout]),
        )
        .multisampling(
            vk::PipelineMultisampleStateCreateInfoBuilder::new()
                .sample_shading_enable(false)
                .rasterization_samples(vk::SampleCountFlagBits::_1),
        )
        .viewport(vk::ViewportBuilder::new()
            .x(0.0)
            .y(0.0)
            .width(extent.width as _)
            .height(extent.height as _)
            .min_depth(0.0)
            .max_depth(1.0)
        )
        .primitive_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .primitive_restart_enable(false)
        .depth_stencil(
            vk::PipelineDepthStencilStateCreateInfoBuilder::new()
                .depth_test_enable(false)
                .depth_write_enable(false)
                .depth_bounds_test_enable(false)
                .depth_compare_op(vk::CompareOp::ALWAYS)
                .min_depth_bounds(0.0)
                .max_depth_bounds(1.0)
                .stencil_test_enable(false),
        )
        .build()
}

pub fn handle_window_resize(pass: &mut RenderPass, vk: &VkContext, fbs: &FramebufferImages) {
    pass.recreate_framebuffers(
        &vk.device,
        vkcore::FramebufferImages {
            width: fbs.luma.extent.width,
            height: fbs.luma.extent.height,
            views: &[fbs.luma.view],
        },
        None,
    );
}
/* use erupt::vk;
use vkcore::{pipeline::Pipeline, RenderPass, VkContext};

use crate::{
    assets,
    renderer::{
        descriptor_sets::{DescriptorSets, SkyPushConstants},
        framebuffers::FramebufferImages,
    },
};

pub fn create_render_pass(
    vk: &VkContext,
    fbs: &FramebufferImages,
) -> anyhow::Result<RenderPass> {
    vk.create_render_pass(vkcore::RenderPassDescriptor {
        color_attachments: &[vkcore::ColorAttachment {
            format: fbs.sky_pass_color.format,
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
            },
        ],
        framebuffer_images: vkcore::FramebufferImages {
            width: fbs.sky_pass_color.extent.width,
            height: fbs.sky_pass_color.extent.height,
            views: &[fbs.sky_pass_color.view],
        },
    })
}

pub fn create_pipelines(
    pass: &RenderPass,
    vk: &VkContext,
    descriptors: &DescriptorSets,
    fbs: &FramebufferImages
) -> anyhow::Result<Pipeline> {
    use vk::ColorComponentFlags as CCF;
    vk.graphics_pipeline_builder()
        .render_pass(pass)
        .vertex_code(assets::postprocess_pipelines::FULLSCREEN_SHADER_VERT)
        .fragment_code(assets::postprocess_pipelines::SKY_SHADER_FRAG)
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
                .push_constant_ranges(&[vk::PushConstantRangeBuilder::new()
                    .offset(0)
                    .size((std::mem::size_of::<SkyPushConstants>()) as _)
                    .stage_flags(vk::ShaderStageFlags::FRAGMENT)])
                .set_layouts(&[
                    descriptors.textures.layout,
                    descriptors.attachments.sky_layout,
                ]),
        )
        .multisampling(
            vk::PipelineMultisampleStateCreateInfoBuilder::new()
                .sample_shading_enable(false)
                .rasterization_samples(vk::SampleCountFlagBits::_1),
        )
        .viewport(
            vk::ViewportBuilder::new()
                .x(0.0)
                .y(0.0)
                .width(fbs.sky_pass_color.extent.width as _)
                .height(fbs.sky_pass_color.extent.height as _)
                .min_depth(0.0)
                .max_depth(1.0),
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
            width: fbs.sky_pass_color.extent.width,
            height: fbs.sky_pass_color.extent.height,
            views: &[fbs.sky_pass_color.view],
        },
        None,
    );
}
 */
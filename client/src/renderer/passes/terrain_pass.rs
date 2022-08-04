use erupt::vk;
use glam::{Mat4, Vec2, Vec3};
use vkcore::{pipeline::Pipeline, RenderPass, VkContext};

use crate::{
    assets,
    renderer::{descriptor_sets::DescriptorSets, framebuffers::FramebufferImages},
};

use anyhow::Result;

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Vertex {
    pub pos: Vec3,
    pub col: Vec3,
    pub uv: Vec2,
}

pub fn create_render_pass(vk: &VkContext, fbs: &FramebufferImages) -> Result<RenderPass> {
    vk.create_render_pass(vkcore::RenderPassDescriptor {
        color_attachments: &[vkcore::ColorAttachment {
            format: fbs.main_pass_color.format,
            load_op: vkcore::LoadOp::CLEAR,
            store_op: vkcore::StoreOp::STORE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }],
        depth_attachment: Some(vkcore::DepthAttachment {
            view: fbs.depth.view,
            format: fbs.depth.format,
            load_op: vkcore::LoadOp::CLEAR,
            store_op: vkcore::StoreOp::STORE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }),
        subpasses: &[vkcore::SubpassDesc {
            color_attachment_refs: &[vkcore::AttachmentRef {
                attachment_idx: 0,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            }],
            input_attachment_refs: &[],
            depth_attachment_ref: Some(vkcore::AttachmentRef {
                attachment_idx: 1,
                layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            }),
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
            vkcore::SubpassDependency {
                src_subpass: vk::SUBPASS_EXTERNAL,
                dst_subpass: 0,
                src_stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                src_access_mask: vk::AccessFlags::empty(),
                dst_stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                dst_access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                dependency_flags: vk::DependencyFlags::BY_REGION,
            },
            vkcore::SubpassDependency {
                src_subpass: 0,
                dst_subpass: vk::SUBPASS_EXTERNAL,
                src_stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                src_access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
                dst_access_mask: vk::AccessFlags::SHADER_READ,
                dependency_flags: vk::DependencyFlags::BY_REGION,
            },
        ],
        framebuffer_images: vkcore::FramebufferImages {
            width: fbs.main_pass_color.extent.width,
            height: fbs.main_pass_color.extent.height,
            views: &[fbs.main_pass_color.view],
        },
    })
}

pub fn create_pipelines(
    pass: &RenderPass,
    vk: &VkContext,
    descriptors: &DescriptorSets
) -> anyhow::Result<Pipeline> {
    use vk::ColorComponentFlags as CCF;
    vk.graphics_pipeline_builder()
        .render_pass(pass)
        .vertex_code(assets::terrain_pipeline::TERRAIN_SHADER_VERT)
        .fragment_code(assets::terrain_pipeline::TERRAIN_SHADER_FRAG)
        .rasterization_state(
            vk::PipelineRasterizationStateCreateInfoBuilder::new()
                .cull_mode(vk::CullModeFlags::BACK)
                .line_width(1.0)
                .polygon_mode(vk::PolygonMode::FILL)
                .depth_bias_enable(false)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .rasterizer_discard_enable(false),
        )
        .input_info(
            vk::PipelineVertexInputStateCreateInfoBuilder::new()
                .vertex_binding_descriptions(&[vk::VertexInputBindingDescriptionBuilder::new()
                    .binding(0)
                    .stride(std::mem::size_of::<Vertex>() as _)
                    .input_rate(vk::VertexInputRate::VERTEX)])
                .vertex_attribute_descriptions(&[
                    vk::VertexInputAttributeDescriptionBuilder::new()
                        .binding(0)
                        .format(vk::Format::R32G32B32_SFLOAT)
                        .offset(0)
                        .location(0),
                    vk::VertexInputAttributeDescriptionBuilder::new()
                        .binding(0)
                        .format(vk::Format::R32G32B32_SFLOAT)
                        .offset(12)
                        .location(1),
                    vk::VertexInputAttributeDescriptionBuilder::new()
                        .binding(0)
                        .format(vk::Format::R32G32_SFLOAT)
                        .offset(24)
                        .location(2),
                ]),
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
                    .size((std::mem::size_of::<Mat4>()) as _)
                    .stage_flags(vk::ShaderStageFlags::VERTEX)])
                .set_layouts(&[descriptors.textures.layout]),
        )
        .multisampling(
            vk::PipelineMultisampleStateCreateInfoBuilder::new()
                .sample_shading_enable(false)
                .rasterization_samples(vk::SampleCountFlagBits::_1),
        )
        .primitive_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .primitive_restart_enable(false)
        .depth_stencil(
            vk::PipelineDepthStencilStateCreateInfoBuilder::new()
                .depth_test_enable(true)
                .depth_write_enable(true)
                .depth_bounds_test_enable(false)
                .depth_compare_op(vk::CompareOp::GREATER_OR_EQUAL)
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
            width: fbs.main_pass_color.extent.width,
            height: fbs.main_pass_color.extent.height,
            views: &[fbs.main_pass_color.view],
        },
        Some(fbs.depth.view),
    );
}

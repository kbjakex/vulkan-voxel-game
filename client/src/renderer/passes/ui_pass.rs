use erupt::vk;
use glam::Vec2;
use vkcore::{pipeline::Pipeline, RenderPass, VkContext};

use crate::{assets, renderer::{descriptor_sets::DescriptorSets}};

use anyhow::Result;

pub struct UiPipelines {
    pub shapes: Pipeline,
    pub text: Pipeline,
}

// 'menu' needs a different initial layout for the image.
// Compatible: https://www.khronos.org/registry/vulkan/specs/1.3-extensions/html/vkspec.html#renderpass-compatibility
pub struct UiRenderPasses {
    pub menu: RenderPass,
    pub game: RenderPass,
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct UiVertex {
    pub x: u16,
    pub y: u16,
    // Either R8G8B8A7 or U16V15.
    // Lsb: 0 => colored, 1 => textured
    pub color_or_uv: u32,
}

impl UiVertex {
    pub fn color(x: u16, y: u16, rgba: u32) -> Self {
        Self {
            x,
            y,
            color_or_uv: rgba,
        }
    }
}

pub fn create_render_pass(vk: &VkContext) -> Result<UiRenderPasses> {
    let extent = vk.swapchain.surface.extent;
    let game = vk.create_render_pass(vkcore::RenderPassDescriptor {
        color_attachments: &[vkcore::ColorAttachment {
            format: vk.swapchain.surface.format.format,
            load_op: vkcore::LoadOp::LOAD,
            store_op: vkcore::StoreOp::STORE,
            initial_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
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
        dependencies: &[vkcore::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            dst_subpass: 0, // first and last subpass
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dependency_flags: vk::DependencyFlags::BY_REGION,
        }],
        framebuffer_images: vkcore::FramebufferImages {
            width: extent.width,
            height: extent.height,
            views: &vk.swapchain.image_views, // will be presented on screen
        },
    })?;

    let menu = vk.create_render_pass(vkcore::RenderPassDescriptor {
        color_attachments: &[vkcore::ColorAttachment {
            format: vk.swapchain.surface.format.format,
            load_op: vkcore::LoadOp::CLEAR,
            store_op: vkcore::StoreOp::STORE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
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
        dependencies: &[vkcore::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            dst_subpass: 0, // first and last subpass
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dependency_flags: vk::DependencyFlags::BY_REGION,
        }],
        framebuffer_images: vkcore::FramebufferImages {
            width: extent.width,
            height: extent.height,
            views: &vk.swapchain.image_views, // will be presented on screen
        },
    })?;

    Ok(UiRenderPasses {
        menu,
        game,
    })
}

pub fn create_pipelines(pass: &RenderPass, vk: &VkContext, descriptors: &DescriptorSets) -> anyhow::Result<UiPipelines> {
    use vk::ColorComponentFlags as CCF;
    let ui_pipeline = vk
        .graphics_pipeline_builder()
        .render_pass(pass)
        .vertex_code(assets::ui_pipeline::IMMEDIATE_MODE_SHADER_VERT)
        .fragment_code(assets::ui_pipeline::IMMEDIATE_MODE_SHADER_FRAG)
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
                .vertex_binding_descriptions(&[vk::VertexInputBindingDescriptionBuilder::new()
                    .binding(0)
                    .stride(std::mem::size_of::<UiVertex>() as _)
                    .input_rate(vk::VertexInputRate::VERTEX)])
                .vertex_attribute_descriptions(&[
                    vk::VertexInputAttributeDescriptionBuilder::new()
                        .binding(0)
                        .format(vk::Format::R32_UINT)
                        .offset(0)
                        .location(0),
                    vk::VertexInputAttributeDescriptionBuilder::new()
                        .binding(0)
                        .format(vk::Format::R32_UINT)
                        .offset(4)
                        .location(1),
                ]),
        )
        .blend_attachment(
            vk::PipelineColorBlendAttachmentStateBuilder::new()
                .blend_enable(true)
                .alpha_blend_op(vk::BlendOp::ADD)
                .color_blend_op(vk::BlendOp::ADD)
                .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .src_alpha_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_write_mask(CCF::R | CCF::G | CCF::B | CCF::A),
        )
        .layout(
            vk::PipelineLayoutCreateInfoBuilder::new()
                .push_constant_ranges(&[vk::PushConstantRangeBuilder::new()
                    .offset(0)
                    .size((std::mem::size_of::<Vec2>()) as _)
                    .stage_flags(vk::ShaderStageFlags::VERTEX)])
                .set_layouts(&[]),
        )
        .primitive_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
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
        .build()?;

    let text_pipeline = vk
        .graphics_pipeline_builder()
        .render_pass(pass)
        .vertex_code(assets::text::TEXT_SHADER_VERT)
        .fragment_code(assets::text::TEXT_SHADER_FRAG)
        .dynamic_states(&[vk::DynamicState::SCISSOR])
        .rasterization_state(
            vk::PipelineRasterizationStateCreateInfoBuilder::new()
                .cull_mode(vk::CullModeFlags::NONE)
                .line_width(1.0)
                .polygon_mode(vk::PolygonMode::FILL)
                .depth_bias_enable(false)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .rasterizer_discard_enable(false),
        )
        .blend_attachment(
            vk::PipelineColorBlendAttachmentStateBuilder::new()
                .blend_enable(true)
                .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .src_alpha_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_blend_op(vk::BlendOp::ADD)
                .color_write_mask(CCF::R | CCF::G | CCF::B | CCF::A),
        )
        .layout(
            vk::PipelineLayoutCreateInfoBuilder::new()
                .push_constant_ranges(&[])
                .set_layouts(&[
                    descriptors.textures.layout,
                    descriptors.text_rendering.layout,
                ]),
        )
        .primitive_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
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
        .build()?;
    
    Ok(UiPipelines {
        shapes: ui_pipeline,
        text: text_pipeline,
    })
}

pub fn handle_window_resize(pass: &mut RenderPass, vk: &VkContext) {
    let extent = vk.swapchain.surface.extent;
    pass.recreate_framebuffers(&vk.device, vkcore::FramebufferImages {
        width: extent.width,
        height: extent.height,
        views: &vk.swapchain.image_views,
    }, None);
}
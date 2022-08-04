use std::ffi::CString;

use erupt::{vk, DeviceLoader};

use crate::{Device, RenderPass, VkContext};

use anyhow::Result;

#[derive(Default, Clone, Copy)]
pub struct Pipeline {
    pub handle: vk::Pipeline,
    pub layout: vk::PipelineLayout,
}

impl Pipeline {
    pub fn null() -> Self {
        Self {
            handle: vk::Pipeline::null(),
            layout: vk::PipelineLayout::null(),
        }
    }

    pub fn destroy_self(&self, device: &Device) {
        unsafe {
            device.destroy_pipeline_layout(self.layout, None);
            device.destroy_pipeline(self.handle, None);
        }
    }
}

pub struct GraphicsPipelineBuilder<'a> {
    vert_shader_code: Option<&'a [u8]>,
    frag_shader_code: Option<&'a [u8]>,
    input_info: vk::PipelineVertexInputStateCreateInfoBuilder<'a>,
    input_assembly: vk::PipelineInputAssemblyStateCreateInfoBuilder<'a>,
    viewport: vk::ViewportBuilder<'a>,
    scissor: vk::Rect2DBuilder<'a>,
    rasterizer: vk::PipelineRasterizationStateCreateInfoBuilder<'a>,
    color_blend_attachment: vk::PipelineColorBlendAttachmentStateBuilder<'a>,
    multisampling: vk::PipelineMultisampleStateCreateInfoBuilder<'a>,
    layout: vk::PipelineLayoutCreateInfoBuilder<'a>,
    depth_stencil: vk::PipelineDepthStencilStateCreateInfoBuilder<'a>,
    dynamic_state: vk::PipelineDynamicStateCreateInfoBuilder<'a>,
    render_pass: Option<&'a RenderPass>,

    vulkan: &'a VkContext,
}

#[allow(unused)]
impl<'a> GraphicsPipelineBuilder<'a> {
    pub fn default(vk: &'a VkContext) -> GraphicsPipelineBuilder<'a> {
        let wnd_extent = vk.swapchain.surface.extent;

        GraphicsPipelineBuilder {
            vert_shader_code: None,
            frag_shader_code: None,
            input_info: vk::PipelineVertexInputStateCreateInfoBuilder::new()
                .vertex_attribute_descriptions(&[])
                .vertex_binding_descriptions(&[]),
            input_assembly: Default::default(),
            viewport: vk::ViewportBuilder::new()
                .x(0.0)
                .y(wnd_extent.height as f32)
                .width(wnd_extent.width as f32)
                .height(-(wnd_extent.height as f32))
                .min_depth(0.0)
                .max_depth(1.0),
            scissor: vk::Rect2DBuilder::new()
                .offset(vk::Offset2D { x: 0, y: 0 })
                .extent(wnd_extent),
            rasterizer: Default::default(),
            color_blend_attachment: Default::default(),
            multisampling: vk::PipelineMultisampleStateCreateInfoBuilder::new()
                .sample_shading_enable(false)
                .rasterization_samples(vk::SampleCountFlagBits::_1),
            layout: Default::default(),
            depth_stencil: Default::default(),
            dynamic_state: Default::default(),
            render_pass: None,

            vulkan: vk,
        }
    }

    pub fn vertex_code(&mut self, code: &'a [u8]) -> &mut Self {
        self.vert_shader_code = Some(code);
        self
    }

    pub fn fragment_code(&mut self, code: &'a [u8]) -> &mut Self {
        self.frag_shader_code = Some(code);
        self
    }

    pub fn scissor(&mut self, scissor: vk::Rect2DBuilder<'a>) -> &mut Self {
        self.scissor = scissor;
        self
    }

    pub fn viewport(&mut self, viewport: vk::ViewportBuilder<'a>) -> &mut Self {
        self.viewport = viewport;
        self
    }

    pub fn primitive_topology(&mut self, topology: vk::PrimitiveTopology) -> &mut Self {
        self.input_assembly.topology = topology;
        self
    }

    pub fn primitive_restart_enable(&mut self, enable: bool) -> &mut Self {
        self.input_assembly.primitive_restart_enable = enable as _;
        self
    }

    pub fn rasterization_state(
        &mut self,
        rasterizer: vk::PipelineRasterizationStateCreateInfoBuilder<'a>,
    ) -> &mut Self {
        self.rasterizer = rasterizer;
        self
    }

    pub fn dynamic_state(
        &mut self,
        state: vk::PipelineDynamicStateCreateInfoBuilder<'a>,
    ) -> &mut Self {
        self.dynamic_state = state;
        self
    }

    pub fn dynamic_states(&mut self, states: &'a [vk::DynamicState]) -> &mut Self {
        self.dynamic_state(vk::PipelineDynamicStateCreateInfoBuilder::new().dynamic_states(states))
    }

    pub fn input_info(
        &mut self,
        info: vk::PipelineVertexInputStateCreateInfoBuilder<'a>,
    ) -> &mut Self {
        self.input_info = info;
        self
    }

    pub fn layout(&mut self, layout: vk::PipelineLayoutCreateInfoBuilder<'a>) -> &mut Self {
        self.layout = layout;
        self
    }

    pub fn depth_stencil(
        &mut self,
        info: vk::PipelineDepthStencilStateCreateInfoBuilder<'a>,
    ) -> &mut Self {
        self.depth_stencil = info;
        self
    }

    pub fn blend_attachment(
        &mut self,
        attachment: vk::PipelineColorBlendAttachmentStateBuilder<'a>,
    ) -> &mut Self {
        self.color_blend_attachment = attachment;
        self
    }

    pub fn multisampling(
        &mut self,
        multisampling: vk::PipelineMultisampleStateCreateInfoBuilder<'a>,
    ) -> &mut Self {
        self.multisampling = multisampling;
        self
    }

    pub fn render_pass(&mut self, render_pass: &'a RenderPass) -> &mut Self {
        self.render_pass = Some(render_pass);
        self
    }

    pub fn build(&self) -> Result<Pipeline> {
        let device = &self.vulkan.device;

        let entry_point = CString::new("main")?;
        let vert_shader = create_shader_module(self.vert_shader_code.unwrap(), device);
        let frag_shader = create_shader_module(self.frag_shader_code.unwrap(), device);

        let shader_stages = &[
            vk::PipelineShaderStageCreateInfoBuilder::new()
                .stage(vk::ShaderStageFlagBits::VERTEX)
                .module(vert_shader)
                .name(&entry_point),
            vk::PipelineShaderStageCreateInfoBuilder::new()
                .stage(vk::ShaderStageFlagBits::FRAGMENT)
                .module(frag_shader)
                .name(&entry_point),
        ];

        let vertex_input = self.input_info;

        let viewports = &[self.viewport];
        let scissors = &[self.scissor];

        let viewport_state = vk::PipelineViewportStateCreateInfoBuilder::new()
            .viewports(viewports)
            .scissors(scissors);

        let attachments = &[self.color_blend_attachment];
        let color_blending = vk::PipelineColorBlendStateCreateInfoBuilder::new()
            .logic_op_enable(false)
            .attachments(attachments);

        let pipeline_layout =
            unsafe { device.create_pipeline_layout(&self.layout, None) }.result()?;

        let pipeline_infos = &[vk::GraphicsPipelineCreateInfoBuilder::new()
            .stages(shader_stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&self.input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&self.rasterizer)
            .multisample_state(&self.multisampling)
            .color_blend_state(&color_blending)
            .layout(pipeline_layout)
            .render_pass(self.render_pass.as_ref().unwrap().handle)
            .depth_stencil_state(&self.depth_stencil)
            .dynamic_state(&self.dynamic_state)
            .subpass(0)];

        let pipeline = unsafe {
            device.create_graphics_pipelines(vk::PipelineCache::null(), pipeline_infos, None)
        }
        .result()?[0];

        unsafe {
            device.destroy_shader_module(vert_shader, None);
            device.destroy_shader_module(frag_shader, None);
        }

        Ok(Pipeline {
            handle: pipeline,
            layout: pipeline_layout,
        })
    }
}

pub struct ComputePipelineBuilder<'a> {
    shader_code: Option<&'a [u8]>,
    layout: vk::PipelineLayoutCreateInfoBuilder<'a>,

    vulkan: &'a VkContext,
}

#[allow(unused)]
impl<'a> ComputePipelineBuilder<'a> {
    pub fn default(vk: &'a VkContext) -> ComputePipelineBuilder<'a> {
        let wnd_extent = vk.swapchain.surface.extent;

        ComputePipelineBuilder {
            shader_code: None,
            layout: Default::default(),

            vulkan: vk,
        }
    }

    pub fn shader(&mut self, code: &'a [u8]) -> &mut Self {
        self.shader_code = Some(code);
        self
    }

    pub fn layout(&mut self, layout: vk::PipelineLayoutCreateInfoBuilder<'a>) -> &mut Self {
        self.layout = layout;
        self
    }

    pub fn build(&self) -> Pipeline {
        let device = &self.vulkan.device;

        let entry_point = CString::new("main").unwrap();
        let shader = create_shader_module(self.shader_code.unwrap(), device);

        let pipeline_layout = unsafe { device.create_pipeline_layout(&self.layout, None) }.unwrap();

        let pipeline_infos = &[vk::ComputePipelineCreateInfoBuilder::new()
            .stage(
                *vk::PipelineShaderStageCreateInfoBuilder::new()
                    .stage(vk::ShaderStageFlagBits::COMPUTE)
                    .module(shader)
                    .name(&entry_point),
            )
            .layout(pipeline_layout)];

        let pipeline = unsafe {
            device.create_compute_pipelines(vk::PipelineCache::null(), pipeline_infos, None)
        }
        .unwrap()[0];

        unsafe {
            device.destroy_shader_module(shader, None);
        }

        Pipeline {
            handle: pipeline,
            layout: pipeline_layout,
        }
    }
}

fn create_shader_module(code: &[u8], device: &DeviceLoader) -> vk::ShaderModule {
    let decoded = erupt::utils::decode_spv(code).unwrap();
    let create_info = vk::ShaderModuleCreateInfoBuilder::new().code(&decoded);

    unsafe { device.create_shader_module(&create_info, None) }.unwrap()
}

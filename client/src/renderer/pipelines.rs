use vkcore::{VkContext, pipeline::Pipeline, Device};

use super::{render_passes::RenderPasses, descriptor_sets::DescriptorSets, passes::ui_pass::UiPipelines};


pub struct Pipelines {
    pub terrain: Pipeline,
    pub fxaa: Pipeline,
    pub luma: Pipeline,
    /* pub sky: Pipeline, */
    pub ui: UiPipelines,
}

impl Pipelines {
    pub fn init(vk: &VkContext, passes: &RenderPasses, descriptors: &DescriptorSets) -> anyhow::Result<Self> {
        use super::passes::*;
        Ok(Self{
            terrain: terrain_pass::create_pipelines(&passes.terrain, vk, descriptors)?,
            fxaa: fxaa_pass::create_pipelines(&passes.fxaa, vk, descriptors)?,
            luma: luminance_pass::create_pipelines(&passes.luma, vk, descriptors)?,
            /* sky: sky_pass::create_pipelines(&passes.sky, vk, descriptors, fbs)?, */
            ui: ui_pass::create_pipelines(&passes.ui.game, vk, descriptors)?,
        })
    }

    pub fn destroy_self(&mut self, device: &Device) {
        self.terrain.destroy_self(device);
        self.fxaa.destroy_self(device);
        self.luma.destroy_self(device);
        /* self.sky.destroy_self(device); */
        self.ui.shapes.destroy_self(device);
        self.ui.text.destroy_self(device);
    }
}


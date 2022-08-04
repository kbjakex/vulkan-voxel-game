use erupt::vk;
use vkcore::{Image, ImageAllocation, VkContext, UsageFlags, VkAllocator, Device};

pub struct FramebufferImages {
    pub main_pass_color: Image,
    /* pub sky_pass_color: Image, */
    pub depth: Image,
    pub luma: Image,
}

impl FramebufferImages {
    pub fn init(vk: &mut VkContext) -> anyhow::Result<Self> {
        let mut ret = Self {
            main_pass_color: Image::null(),
            /* sky_pass_color: Image::null(), */
            depth: Image::null(),
            luma: Image::null(),
        };

        ret.handle_window_resize(vk)?;

        Ok(ret)
    }

    pub fn handle_window_resize(&mut self, vk: &mut VkContext) -> anyhow::Result<()> {
        // Deallocate old ones first so that there won't be 2x total memory required
        if !self.main_pass_color.view.is_null() {
            self.destroy_self(&vk.device, &mut vk.allocator)?;
        }

        self.main_pass_color = alloc_color_fb(vk)?;
        /* self.sky_pass_color = alloc_color_fb(vk)?; */
        self.depth = vk.allocator.allocate_image(
            &vk.device,
            &ImageAllocation {
                format: vk::Format::D32_SFLOAT,
                layers: 1,
                mip_levels: 1,
                extent: vk.swapchain.surface.extent,
                usage: UsageFlags::FAST_DEVICE_ACCESS,
                flags: vk::ImageAspectFlags::DEPTH,
                vk_usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            },
        )?;
        self.luma = vk.allocator.allocate_image(
            &vk.device,
            &ImageAllocation {
                format: vk::Format::R8_UNORM,
                layers: 1,
                mip_levels: 1,
                extent: vk.swapchain.surface.extent,
                usage: UsageFlags::FAST_DEVICE_ACCESS,
                flags: vk::ImageAspectFlags::COLOR,
                vk_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            },
        )?;

        Ok(())
    }

    pub fn destroy_self(&mut self, device: &Device, allocator: &mut VkAllocator) -> anyhow::Result<()> {
        allocator.deallocate_image(&mut self.main_pass_color, device)?;
        /* allocator.deallocate_image(&mut self.sky_pass_color, device)?; */
        allocator.deallocate_image(&mut self.depth, device)?;
        allocator.deallocate_image(&mut self.luma, device)?;
        Ok(())
    }
}

fn alloc_color_fb(vk: &mut VkContext) -> anyhow::Result<Image> {
    vk.allocator.allocate_image(
        &vk.device,
        &ImageAllocation {
            format: vk::Format::R8G8B8A8_UNORM,
            layers: 1,
            mip_levels: 1,
            extent: vk.swapchain.surface.extent,
            usage: UsageFlags::FAST_DEVICE_ACCESS,
            flags: vk::ImageAspectFlags::COLOR,
            vk_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        },
    )
}
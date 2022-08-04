
use erupt::vk;
use glam::{Vec2, Mat4};
use vkcore::{BufferAllocation, Uploader, Device, VkAllocator, Image, ImageAllocation, Buffer, UsageFlags, VkContext};

use anyhow::Result;

use crate::assets;

use super::renderer::FRAMES_IN_FLIGHT;

pub struct DescriptorSets {
    pub pool: vk::DescriptorPool,

    pub textures: Textures,
    pub text_rendering: TextBuffers,
    pub attachments: InputAttachments,
}

impl DescriptorSets {
    pub fn create(vk: &mut VkContext) -> Result<DescriptorSets> {
        println!("CREATING DESCRIPTOR SETS");
        let pool = unsafe {
            vk.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfoBuilder::new()
                    .max_sets(10)
                    .pool_sizes(&[
                        vk::DescriptorPoolSizeBuilder::new()
                            ._type(vk::DescriptorType::UNIFORM_BUFFER)
                            .descriptor_count(10),
                        vk::DescriptorPoolSizeBuilder::new()
                            ._type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                            .descriptor_count(1),
                    ]),
                None,
            )
        }
        .result()?;
    
        let textures = Textures::create(&vk.device, pool, &mut vk.uploader, &mut vk.allocator)?;
        let text_rendering = TextBuffers::create(&vk.device, pool)?;
        let attachments = InputAttachments::create(&vk.device, pool, &mut vk.allocator)?;
    
        Ok(DescriptorSets {
            pool,
            textures,
            text_rendering,
            attachments,
        })
    }

    pub fn destroy_self(&mut self, device: &Device, alloc: &mut VkAllocator) -> Result<()> {
        self.textures.destroy_self(device, alloc)?;
        self.text_rendering.destroy_self(device)?;
        self.attachments.destroy_self(device, alloc)?;

        unsafe {
            device.destroy_descriptor_pool(self.pool, None);
        }
        println!("All descriptor sets destroyed");

        Ok(())
    }
}

pub struct Textures {
    pub layout: vk::DescriptorSetLayout,
    pub descriptor_set: vk::DescriptorSet,

    pub sampler: vk::Sampler,
    pub texture: Image,

    pub text_sampler: vk::Sampler,
    pub text_texture: Image,
}
impl Textures {
    fn create(
        device: &Device,
        pool: vk::DescriptorPool,
        uploader: &mut Uploader,
        allocator: &mut VkAllocator,
    ) -> Result<Self> {
        let layout = unsafe {
            device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfoBuilder::new().bindings(&[
                    vk::DescriptorSetLayoutBindingBuilder::new()
                        .binding(0)
                        .descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBindingBuilder::new()
                        .binding(1)
                        .descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT),
                ]),
                None,
            )
        }
        .result()?;

        let descriptor_set = unsafe {
            device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfoBuilder::new()
                    .descriptor_pool(pool)
                    .set_layouts(&[layout]),
            )
        }
        .result()?[0];

        let sampler = unsafe {
            device.create_sampler(
                &vk::SamplerCreateInfoBuilder::new()
                    .min_filter(vk::Filter::NEAREST)
                    .mag_filter(vk::Filter::NEAREST)
                    .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                    .anisotropy_enable(false)
                    .max_anisotropy(8.0)
                    .mip_lod_bias(0.0)
                    .min_lod(0.0)
                    .max_lod(5.0),
                None,
            )
        }
        .result()?;

        let texture = Self::load_texture_array(device, uploader, allocator)?;

        let text_sampler = unsafe {
            device.create_sampler(
                &vk::SamplerCreateInfoBuilder::new()
                    .min_filter(vk::Filter::NEAREST)
                    .mag_filter(vk::Filter::NEAREST)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .anisotropy_enable(false)
                    .max_anisotropy(0.0)
                    .mip_lod_bias(0.0)
                    .min_lod(0.0)
                    .max_lod(0.0),
                None,
            )
        }
        .result()?;

        let text_texture = Self::load_text_atlas(device, uploader, allocator)?;

        unsafe {
            device.update_descriptor_sets(
                &[vk::WriteDescriptorSetBuilder::new()
                    .dst_binding(0)
                    .dst_set(descriptor_set)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&[vk::DescriptorImageInfoBuilder::new()
                        .image_view(texture.view)
                        .sampler(sampler)
                        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]),
                    vk::WriteDescriptorSetBuilder::new()
                    .dst_binding(1)
                    .dst_set(descriptor_set)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&[vk::DescriptorImageInfoBuilder::new()
                        .image_view(text_texture.view)
                        .sampler(text_sampler)
                        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)])],
                &[],
            );
        }

        Ok(Self {
            layout,
            descriptor_set,
            sampler,
            texture,
            text_sampler,
            text_texture,
        })
    }

    fn load_texture_array(
        device: &Device,
        uploader: &mut Uploader,
        allocator: &mut VkAllocator,
    ) -> Result<Image> {
        let bytes = lz4::block::decompress(assets::textures::TEXTURES, None)?;

        let layers = bytes.len() as u32 / (16 * 16 * 4);
        let mip_levels = (16u32).trailing_zeros() + 1; // floor(log2())
        println!("Mip levels for {} textures: {}", layers, mip_levels);

        println!("Found {} layers", layers);
        let mut img = allocator.allocate_image(
            device,
            &ImageAllocation {
                format: vk::Format::R8G8B8A8_SRGB,
                layers,
                mip_levels,
                extent: vk::Extent2D {
                    width: 16,
                    height: 16,
                },
                usage: UsageFlags::FAST_DEVICE_ACCESS,
                flags: vk::ImageAspectFlags::COLOR,
                vk_usage: vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST,
            },
        )?;
        uploader.upload_to_image(
            device,
            &bytes,
            &mut img,
            *vk::ImageSubresourceRangeBuilder::new()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(mip_levels)
                .base_array_layer(0)
                .layer_count(layers),
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            true,
        )?;
        Ok(img)
    }

    fn load_text_atlas(device: &Device, uploader: &mut Uploader, allocator: &mut VkAllocator) -> Result<Image> {
        let data = lz4::block::decompress(assets::text::TEXTURE_ATLAS, None)?;

        let mut img = allocator.allocate_image(
            &device,
            &ImageAllocation {
                format: vk::Format::R8_UNORM,
                layers: 1,
                mip_levels: 1,
                extent: vk::Extent2D {
                    width: 16*8,
                    height: 16*8,
                },
                usage: UsageFlags::FAST_DEVICE_ACCESS,
                flags: vk::ImageAspectFlags::COLOR,
                vk_usage: vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
            },
        )?;

        uploader.upload_to_image(
            &device,
            &data,
            &mut img,
            *vk::ImageSubresourceRangeBuilder::new()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1),
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            false,
        )?;

        Ok(img)
    }

    pub fn destroy_self(&mut self, device: &Device, alloc: &mut VkAllocator) -> Result<()> {
        println!("Textures (descriptor sets) destroyed");
        alloc.deallocate_image(&mut self.texture, device)?;
        alloc.deallocate_image(&mut self.text_texture, device)?;
        unsafe {
            device.destroy_sampler(self.sampler, None);
            device.destroy_sampler(self.text_sampler, None);
            device.destroy_descriptor_set_layout(self.layout, None);
        }
        Ok(())
    }
}

pub struct TextBuffers {
    pub layout: vk::DescriptorSetLayout,
    pub descriptor_sets: Vec<vk::DescriptorSet>,
}

impl TextBuffers {
    fn create(
        device: &Device,
        pool: vk::DescriptorPool,
    ) -> Result<Self> {
        let layout = unsafe {
            device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfoBuilder::new().bindings(&[
                    vk::DescriptorSetLayoutBindingBuilder::new()
                        .binding(0)
                        .descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .stage_flags(vk::ShaderStageFlags::VERTEX),
                    vk::DescriptorSetLayoutBindingBuilder::new()
                        .binding(1)
                        .descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .stage_flags(vk::ShaderStageFlags::VERTEX),
                ]),
                None,
            )
        }
        .result()?;

        let layouts = [layout; FRAMES_IN_FLIGHT as usize];

        let descriptor_sets = unsafe {
            device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfoBuilder::new()
                    .descriptor_pool(pool)
                    .set_layouts(&layouts),
            )
        }
        .result()?;

        Ok(Self {
            layout,
            descriptor_sets: descriptor_sets.to_vec(),
        })
    }

    pub fn destroy_self(&mut self, device: &Device) -> Result<()> {
        println!("TextBuffers (descriptor sets) destroyed");
        unsafe {
            device.destroy_descriptor_set_layout(self.layout, None);
        }
        Ok(())
    }
}

pub struct InputAttachments {
    pub fxaa_layout: vk::DescriptorSetLayout,
    pub fxaa_descriptor_set: vk::DescriptorSet,
    pub fxaa_ubo_buf: Buffer,

    /* pub sky_layout: vk::DescriptorSetLayout,
    pub sky_descriptor_set: vk::DescriptorSet, */

    pub luma_layout: vk::DescriptorSetLayout,
    pub luma_descriptor_set: vk::DescriptorSet,

    pub sampler: vk::Sampler,
}

#[repr(C)]
pub struct TexelSizeUBO {
    pub texel_size: Vec2,
}

#[repr(C)]
pub struct SkyPushConstants {
    pub inv_proj_view: Mat4,
    pub sun_azimuth: f32,
}

impl InputAttachments {
    fn create(
        device: &Device,
        pool: vk::DescriptorPool,
        allocator: &mut VkAllocator,
    ) -> Result<Self> {
        let fxaa_layout = unsafe {
            device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfoBuilder::new().bindings(&[
                    vk::DescriptorSetLayoutBindingBuilder::new()
                        .binding(0)
                        .descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBindingBuilder::new()
                        .binding(1)
                        .descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBindingBuilder::new()
                        .binding(2)
                        .descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT),
                ]),
                None,
            )
        }
        .result()?;

        let fxaa_descriptor_set = unsafe {
            device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfoBuilder::new()
                    .descriptor_pool(pool)
                    .set_layouts(&[fxaa_layout]),
            )
        }
        .result()?[0];

        let luma_layout = unsafe {
            device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfoBuilder::new().bindings(&[
                    vk::DescriptorSetLayoutBindingBuilder::new()
                        .binding(0)
                        .descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT),
                ]),
                None,
            )
        }
        .result()?;

        let luma_descriptor_set = unsafe {
            device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfoBuilder::new()
                    .descriptor_pool(pool)
                    .set_layouts(&[luma_layout]),
            )
        }
        .result()?[0];

        /* let sky_layout = unsafe {
            device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfoBuilder::new().bindings(&[
                    vk::DescriptorSetLayoutBindingBuilder::new()
                        .binding(0)
                        .descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBindingBuilder::new()
                        .binding(1)
                        .descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT),
                ]),
                None,
            )
        }
        .result()?; */

        /* let sky_descriptor_set = unsafe {
            device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfoBuilder::new()
                    .descriptor_pool(pool)
                    .set_layouts(&[sky_layout]),
            )
        }
        .result()?[0]; */

        let sampler = unsafe {
            device.create_sampler(
                &vk::SamplerCreateInfoBuilder::new()
                    .min_filter(vk::Filter::LINEAR)
                    .mag_filter(vk::Filter::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )
        }
        .result()?;

        let fxaa_ubo_buf = allocator.allocate_buffer(
            device,
            &BufferAllocation {
                size: std::mem::size_of::<TexelSizeUBO>(),
                usage: UsageFlags::HOST_ACCESS,
                vk_usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
            },
        )?;

        Ok(Self {
            fxaa_layout,
            fxaa_descriptor_set,
            fxaa_ubo_buf,
            luma_layout,
            luma_descriptor_set,
            /* sky_layout,
            sky_descriptor_set, */
            sampler,
        })
    }

    pub fn destroy_self(&mut self, device: &Device, allocator: &mut VkAllocator) -> Result<()> {
        println!("InputAttachments (descriptor sets) destroyed");
        unsafe {
            allocator.deallocate_buffer(&mut self.fxaa_ubo_buf, device)?;
            device.destroy_sampler(self.sampler, None);
            device.destroy_descriptor_set_layout(self.fxaa_layout, None);
            /* device.destroy_descriptor_set_layout(self.sky_layout, None); */
            device.destroy_descriptor_set_layout(self.luma_layout, None);
        }
        Ok(())
    }
}

use erupt::{vk, InstanceLoader};
use gpu_alloc::{GpuAllocator, MemoryBlock, Request};

use crate::Device;
use anyhow::{bail, Result};
use gpu_alloc_erupt::{device_properties, EruptMemoryDevice};

type VulkanAllocator = GpuAllocator<vk::DeviceMemory>;

pub struct Image {
    pub handle: vk::Image,
    pub view: vk::ImageView,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
    pub layers: u32,
    pub mip_levels: u32,
    pub mem: Option<MemoryBlock<vk::DeviceMemory>>,
}

impl Image {
    pub fn null() -> Self {
        Image {
            handle: vk::Image::null(),
            view: vk::ImageView::null(),
            format: vk::Format::UNDEFINED,
            layers: 1,
            mip_levels: 1,
            extent: vk::Extent2D {
                width: 0,
                height: 0,
            },
            mem: None,
        }
    }
}

pub struct Buffer {
    pub handle: vk::Buffer,
    pub size: u64,
    pub mem: Option<MemoryBlock<vk::DeviceMemory>>,
}

impl Default for Buffer {
    fn default() -> Self {
        Self::null()
    }
}

impl Buffer {
    pub fn null() -> Self {
        Buffer {
            handle: vk::Buffer::null(),
            size: 0,
            mem: None,
        }
    }
}

pub struct VkAllocator {
    handle: VulkanAllocator,
}

impl VkAllocator{
    pub fn new(device: &Device, instance: &InstanceLoader) -> Result<Self> {
        let allocator = {
            let mut props = unsafe { device_properties(instance, device.physical) }?;
            props.buffer_device_address = false;
            VulkanAllocator::new(gpu_alloc::Config::i_am_prototyping(), props)
        };

        Ok(VkAllocator { handle: allocator })
    }

    pub fn deallocate_image(&mut self, image: &mut Image, device: &Device) -> Result<()> {
        if let Some(mem) = image.mem.take() {
            unsafe {
                device.destroy_image_view(image.view, None);
                device.destroy_image(image.handle, None);
                self.handle.dealloc(EruptMemoryDevice::wrap(device), mem);

                *image = Image::null();
            }
            Ok(())
        } else {
            bail!("Tried to free a non-allocated buffer!");
        }
    }

    pub fn deallocate_buffer(&mut self, buffer: &mut Buffer, device: &Device) -> Result<()> {
        if let Some(mem) = buffer.mem.take() {
            unsafe {
                device.destroy_buffer(buffer.handle, None);
                self.handle.dealloc(EruptMemoryDevice::wrap(device), mem);

                *buffer = Buffer::null();
            }
            Ok(())
        } else {
            bail!("Tried to free a non-allocated buffer!");
        }
    }

    pub fn allocate_buffer(&mut self, device: &Device, alloc: &BufferAllocation) -> Result<Buffer> {
        let usage = if alloc.usage == UsageFlags::FAST_DEVICE_ACCESS
        /* && !device.integrated */
        {
            // For VRAM, a copy from staging buffer is required, and copy destination
            // buffers/images require the TRANSFER_DST bit in usage
            alloc.vk_usage | vk::BufferUsageFlags::TRANSFER_DST
        } else {
            alloc.vk_usage
        };

        let buf = unsafe {
            device.create_buffer(
                &vk::BufferCreateInfoBuilder::new()
                    .usage(usage)
                    .size(alloc.size as u64)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
        }
        .result()?;

        let mem_reqs = unsafe { device.get_buffer_memory_requirements(buf) };
        let request = Request {
            size: mem_reqs.size as u64,
            align_mask: mem_reqs.alignment - 1,
            usage: alloc.usage,
            memory_types: mem_reqs.memory_type_bits,
        };

        let mem = unsafe { self.handle.alloc(EruptMemoryDevice::wrap(device), request) }?;

        println!(
            "Allocated {} bytes of memory with alignment of {} and memory type {}. Offset: {}",
            mem_reqs.size,
            mem_reqs.alignment,
            mem_reqs.memory_type_bits,
            mem.offset()
        );

        unsafe {
            device
                .bind_buffer_memory(buf, *mem.memory(), mem.offset())
                .result()?;
        }

        Ok(Buffer {
            handle: buf,
            size: request.size,
            mem: Some(mem),
        })
    }

    /// NEED to explicitly add `vk::ImageUsageFlags::TRANSFER_DST` to `vk_flags` if uploaded from CPU!
    pub fn allocate_image(&mut self, device: &Device, alloc: &ImageAllocation) -> Result<Image> {
        let img = unsafe {
            device.create_image(
                &vk::ImageCreateInfoBuilder::new()
                    .image_type(vk::ImageType::_2D)
                    .format(alloc.format)
                    .extent(vk::Extent3D {
                        width: alloc.extent.width,
                        height: alloc.extent.height,
                        depth: 1,
                    })
                    .mip_levels(alloc.mip_levels)
                    .array_layers(alloc.layers)
                    .samples(vk::SampleCountFlagBits::_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(alloc.vk_usage)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
        }
        .result()?;

        let mem_reqs = unsafe { device.get_image_memory_requirements(img) };
        println!("Mem reqs for {}x{} image with {} layers, {} mip levels, alignment of {} and format {:?} is {} bytes", alloc.extent.width, alloc.extent.height, alloc.layers, alloc.mip_levels, mem_reqs.alignment, alloc.format, mem_reqs.size);

        let img_mem = unsafe {
            self.handle.alloc(
                EruptMemoryDevice::wrap(device),
                Request {
                    size: mem_reqs.size,
                    align_mask: mem_reqs.alignment - 1,
                    usage: alloc.usage,
                    memory_types: mem_reqs.memory_type_bits,
                },
            )
        }?;

        unsafe {
            device
                .bind_image_memory(img, *img_mem.memory(), img_mem.offset())
                .result()?;
        }

        let view = {
            let view_type = if alloc.layers > 1 {
                vk::ImageViewType::_2D_ARRAY
            } else {
                vk::ImageViewType::_2D
            };

            unsafe {
                device.create_image_view(
                    &vk::ImageViewCreateInfoBuilder::new()
                        .view_type(view_type)
                        .image(img)
                        .format(alloc.format)
                        .subresource_range(
                            *vk::ImageSubresourceRangeBuilder::new()
                                .base_mip_level(0)
                                .level_count(alloc.mip_levels)
                                .base_array_layer(0)
                                .layer_count(alloc.layers)
                                .aspect_mask(alloc.flags),
                        ),
                    None,
                )
            }
            .result()?
        };

        Ok(Image {
            handle: img,
            view,
            format: alloc.format,
            extent: alloc.extent,
            layers: alloc.layers,
            mip_levels: alloc.mip_levels,
            mem: Some(img_mem),
        })
    }
}

pub type UsageFlags = gpu_alloc::UsageFlags;

pub struct BufferAllocation {
    pub size: usize,
    pub usage: UsageFlags,
    pub vk_usage: vk::BufferUsageFlags,
}

pub struct ImageAllocation {
    pub format: vk::Format,
    pub layers: u32,
    pub mip_levels: u32,
    pub extent: vk::Extent2D,
    pub usage: UsageFlags,
    pub flags: vk::ImageAspectFlags,
    pub vk_usage: vk::ImageUsageFlags,
}

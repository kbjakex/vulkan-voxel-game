use erupt::vk;

use anyhow::{bail, Result};
use gpu_alloc::UsageFlags;
use gpu_alloc_erupt::EruptMemoryDevice;

use crate::{Buffer, BufferAllocation, Device, Image, VkAllocator};

const STAGING_BUFFER_SIZE: usize = 1 << 24; // 16 MiB (same as Sodium)

#[derive(Clone, Copy)]
enum MemCopyOp {
    Buf2Buffer {
        dst: vk::Buffer,
        src_offset: u32,
        dst_offset: u32,
        size: u32,
    },
    Buf2Image {
        dst: vk::Image,
        extent: vk::Extent2D,
        range: vk::ImageSubresourceRange,
        shader_stages: vk::PipelineStageFlags,
        src_offset: u32,
    },
}

struct MipGenData {
    image: vk::Image,
    size: vk::Extent2D,
    range: vk::ImageSubresourceRange,
}

pub struct Uploader {
    pool: vk::CommandPool,
    commands: vk::CommandBuffer,

    upload_fence: vk::Fence,

    staging_buffer: Buffer,
    staging_buffer_head: u32,
    pending_copy_ops: Vec<MemCopyOp>,
    pending_mip_gens: Vec<MipGenData>,

    wait_needed: bool,
}

impl Uploader {
    pub fn new(device: &Device, allocator: &mut VkAllocator) -> Result<Self> {
        let fence_info = vk::FenceCreateInfoBuilder::new();
        let fence = unsafe { device.create_fence(&fence_info, None) }.result()?;

        let cmd_pool_info =
            vk::CommandPoolCreateInfoBuilder::new().queue_family_index(device.queue.family_idx);

        let cmd_pool = unsafe { device.create_command_pool(&cmd_pool_info, None) }.result()?;
        let cmd_buf_allocate_info = vk::CommandBufferAllocateInfoBuilder::new()
            .command_pool(cmd_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let cmds = unsafe { device.allocate_command_buffers(&cmd_buf_allocate_info) }.result()?;

        println!("[uploader.rs] Allocating staging buffer");
        let staging_buf = allocator.allocate_buffer(
            device,
            &BufferAllocation {
                size: STAGING_BUFFER_SIZE,
                usage: UsageFlags::UPLOAD,
                vk_usage: vk::BufferUsageFlags::TRANSFER_SRC,
            },
        )?;

        Ok(Uploader {
            pool: cmd_pool,
            commands: cmds[0],
            upload_fence: fence,
            staging_buffer: staging_buf,
            staging_buffer_head: 0,
            pending_copy_ops: Vec::new(),
            pending_mip_gens: Vec::new(),
            wait_needed: false,
        })
    }

    pub fn destroy_self(&mut self, device: &Device, allocator: &mut VkAllocator) -> Result<()> {
        allocator.deallocate_buffer(&mut self.staging_buffer, device)?;

        unsafe {
            device.destroy_fence(self.upload_fence, None);
            device.destroy_command_pool(self.pool, None);
        }
        Ok(())
    }

    pub fn upload_to_image(
        &mut self,
        device: &Device,
        data: &[u8],
        dst_image: &mut Image,
        range: vk::ImageSubresourceRange,
        stages: vk::PipelineStageFlags,
        gen_mips: bool,
    ) -> Result<()> {
        if self.staging_buffer_head as u64 + data.len() as u64 >= self.staging_buffer.size {
            bail!(
                "Staging buffer ran out of space while uploading image! Uploaded {} bytes, head was at {}/{}",
                data.len(),
                self.staging_buffer_head,
                self.staging_buffer.size
            );
        }

        unsafe {
            self.staging_buffer.mem.as_mut().unwrap().write_bytes(
                EruptMemoryDevice::wrap(device),
                self.staging_buffer_head as _,
                data,
            )
        }?;

        self.pending_copy_ops.push(MemCopyOp::Buf2Image {
            dst: dst_image.handle,
            extent: dst_image.extent,
            range,
            shader_stages: stages,
            src_offset: self.staging_buffer_head,
        });
        self.staging_buffer_head += data.len() as u32;

        if gen_mips {
            self.pending_mip_gens.push(MipGenData {
                image: dst_image.handle,
                size: dst_image.extent,
                range,
            });
        }

        Ok(())
    }

    pub fn upload_to_buffer<T: Sized>(
        &mut self,
        device: &Device,
        data: &[T],
        dst_buf: &mut Buffer,
        dst_buf_offset: u32,
    ) -> Result<()> {
        let n_bytes = data.len() * std::mem::size_of::<T>();
        let bytes =
            unsafe { std::slice::from_raw_parts::<u8>(data.as_ptr() as *const u8, n_bytes) };

        self.upload_bytes_to_buffer(device, bytes, dst_buf, dst_buf_offset)
    }

    pub fn upload_bytes_to_buffer(
        &mut self,
        device: &Device,
        data: &[u8],
        dst_buf: &mut Buffer,
        dst_buf_offset: u32,
    ) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        let mem = match dst_buf.mem {
            Some(ref mut mem) => mem,
            None => {
                bail!("Tried to upload to unallocated buffer!");
            }
        };
        if mem
            .props()
            .contains(gpu_alloc::MemoryPropertyFlags::HOST_VISIBLE)
        {
            // Staging buffer not needed, direct upload.
            unsafe { mem.write_bytes(EruptMemoryDevice::wrap(device), dst_buf_offset as _, data) }?;
            return Ok(());
        }

        if self.staging_buffer_head as u64 + data.len() as u64 >= self.staging_buffer.size {
            bail!(
                "Staging buffer ran out of space! Uploaded {} bytes, head was at {}/{}",
                data.len(),
                self.staging_buffer_head,
                self.staging_buffer.size
            );
        }

        unsafe {
            self.staging_buffer.mem.as_mut().unwrap().write_bytes(
                EruptMemoryDevice::wrap(device),
                self.staging_buffer_head as _,
                data,
            )
        }?;

        self.pending_copy_ops.push(MemCopyOp::Buf2Buffer {
            dst: dst_buf.handle,
            src_offset: self.staging_buffer_head,
            dst_offset: dst_buf_offset,
            size: data.len() as _,
        });
        self.staging_buffer_head += data.len() as u32;

        Ok(())
    }

    pub fn flush_staged(&mut self, device: &Device) -> Result<()> {
        self.wait_fence_if_unfinished(device)?;
        unsafe { device.reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty()) }
            .result()?;

        unsafe {
            device.begin_command_buffer(
                self.commands,
                &vk::CommandBufferBeginInfoBuilder::new()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
        }
        .result()?;

        let cmd = self.commands;
        let staging = &self.staging_buffer;
        for &task in &self.pending_copy_ops {
            match task {
                MemCopyOp::Buf2Buffer {
                    dst,
                    src_offset,
                    dst_offset,
                    size,
                } => unsafe {
                    device.cmd_copy_buffer(
                        cmd,
                        staging.handle,
                        dst,
                        &[vk::BufferCopyBuilder::new()
                            .dst_offset(dst_offset as _)
                            .src_offset(src_offset as _)
                            .size(size as _)],
                    );
                },
                MemCopyOp::Buf2Image {
                    dst,
                    extent,
                    range,
                    shader_stages,
                    src_offset,
                } => unsafe {
                    device.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                        vk::PipelineStageFlags::TRANSFER,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[vk::ImageMemoryBarrierBuilder::new()
                            .image(dst)
                            .old_layout(vk::ImageLayout::UNDEFINED)
                            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                            .src_access_mask(vk::AccessFlags::empty())
                            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                            .subresource_range(range)],
                    );
                    device.cmd_copy_buffer_to_image(
                        cmd,
                        staging.handle,
                        dst,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[vk::BufferImageCopyBuilder::new()
                            .buffer_offset(src_offset as _)
                            .buffer_row_length(0)
                            .buffer_image_height(0)
                            .image_extent(vk::Extent3D {
                                width: extent.width,
                                height: extent.height,
                                depth: 1,
                            })
                            .image_subresource(vk::ImageSubresourceLayers {
                                aspect_mask: vk::ImageAspectFlags::COLOR,
                                mip_level: range.base_mip_level,
                                base_array_layer: range.base_array_layer,
                                layer_count: range.layer_count,
                            })],
                    );
                    device.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::TRANSFER,
                        shader_stages,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[vk::ImageMemoryBarrierBuilder::new()
                            .image(dst)
                            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                            .dst_access_mask(vk::AccessFlags::SHADER_READ)
                            .subresource_range(range)],
                    );
                },
            }
        }

        unsafe { device.end_command_buffer(self.commands) }.result()?;

        unsafe {
            device.queue_submit(
                *device.queue,
                &[vk::SubmitInfoBuilder::new().command_buffers(&[self.commands])],
                self.upload_fence,
            )
        }
        .result()?;
        self.wait_needed = true;
        self.pending_copy_ops.clear();
        self.staging_buffer_head = 0;

        if self.pending_mip_gens.is_empty() {
            return Ok(());
        }
        // wait immediately
        self.wait_fence_if_unfinished(device)?;

        unsafe { device.reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty()) }
            .result()?;

        unsafe {
            device.begin_command_buffer(
                self.commands,
                &vk::CommandBufferBeginInfoBuilder::new()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
        }
        .result()?;

        for mip_gen_ops in &self.pending_mip_gens {
            unsafe {
                device.cmd_pipeline_barrier(self.commands,
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[vk::ImageMemoryBarrierBuilder::new()
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .image(mip_gen_ops.image)
                        .subresource_range(mip_gen_ops.range
                        )
                    ]
                );
            }

            let mut barrier = vk::ImageMemoryBarrierBuilder::new()
                .image(mip_gen_ops.image)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .subresource_range(
                    *vk::ImageSubresourceRangeBuilder::new()
                        .aspect_mask(mip_gen_ops.range.aspect_mask)
                        .base_array_layer(0)
                        .layer_count(1)
                        .level_count(1),
                );

            for layer in 0..mip_gen_ops.range.layer_count {
                barrier.subresource_range.base_array_layer = layer;
                let mut mip_width = mip_gen_ops.size.width;
                let mut mip_height = mip_gen_ops.size.height;
                for level in 1..mip_gen_ops.range.level_count {
                    barrier.subresource_range.base_mip_level = level - 1;
                    barrier.old_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
                    barrier.new_layout = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
                    barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
                    barrier.dst_access_mask = vk::AccessFlags::TRANSFER_READ;

                    unsafe {
                        device.cmd_pipeline_barrier(
                            self.commands,
                            vk::PipelineStageFlags::TRANSFER,
                            vk::PipelineStageFlags::TRANSFER,
                            vk::DependencyFlags::empty(),
                            &[],
                            &[],
                            &[barrier],
                        );
                    }

                    let sub_width = (mip_width / 2).max(1);
                    let sub_height = (mip_height / 2).max(1);

                    let blit = vk::ImageBlitBuilder::new()
                        .src_offsets([
                            *vk::Offset3DBuilder::new().x(0).y(0).z(0),
                            *vk::Offset3DBuilder::new()
                                .x(mip_width as _)
                                .y(mip_height as _)
                                .z(1),
                        ])
                        .src_subresource(
                            *vk::ImageSubresourceLayersBuilder::new()
                                .aspect_mask(mip_gen_ops.range.aspect_mask)
                                .mip_level(level -1)
                                .base_array_layer(layer)
                                .layer_count(1),
                        )
                        .dst_offsets([
                            *vk::Offset3DBuilder::new().x(0).y(0).z(0),
                            *vk::Offset3DBuilder::new()
                                .x(sub_width as _)
                                .y(sub_height as _)
                                .z(1),
                        ])
                        .dst_subresource(
                            *vk::ImageSubresourceLayersBuilder::new()
                                .aspect_mask(mip_gen_ops.range.aspect_mask)
                                .mip_level(level as _)
                                .base_array_layer(layer)
                                .layer_count(1),
                        );

                    unsafe {
                        device.cmd_blit_image(
                            self.commands,
                            mip_gen_ops.image,
                            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                            mip_gen_ops.image,
                            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                            &[blit],
                            vk::Filter::LINEAR,
                        );
                    }

                    barrier.old_layout = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
                    barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
                    barrier.src_access_mask = vk::AccessFlags::TRANSFER_READ;
                    barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

                    unsafe {
                        device.cmd_pipeline_barrier(
                            self.commands,
                            vk::PipelineStageFlags::TRANSFER,
                            vk::PipelineStageFlags::FRAGMENT_SHADER,
                            vk::DependencyFlags::empty(),
                            &[],
                            &[],
                            &[barrier],
                        );
                    }

                    if mip_width > 1 {
                        mip_width /= 2;
                    }
                    if mip_height > 1 {
                        mip_height /= 2;
                    }
                }
                barrier.subresource_range.base_mip_level = mip_gen_ops.range.level_count - 1;
                barrier.old_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
                barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
                barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
                barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

                unsafe {
                    device.cmd_pipeline_barrier(
                        self.commands,
                        vk::PipelineStageFlags::TRANSFER,
                        vk::PipelineStageFlags::FRAGMENT_SHADER,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[barrier],
                    );
                }
            }
        }


        unsafe { device.end_command_buffer(self.commands) }.result()?;

        unsafe {
            device.queue_submit(
                *device.queue,
                &[vk::SubmitInfoBuilder::new().command_buffers(&[self.commands])],
                self.upload_fence,
            )
        }
        .result()?;
        self.wait_needed = true;
        self.pending_mip_gens.clear();

        Ok(())
    }

    pub fn wait_fence_if_unfinished(&mut self, device: &Device) -> Result<()> {
        if self.wait_needed {
            unsafe { device.wait_for_fences(&[self.upload_fence], true, u64::MAX) }.result()?;
            unsafe { device.reset_fences(&[self.upload_fence]) }.result()?;
            self.wait_needed = false;
        }
        Ok(())
    }
}

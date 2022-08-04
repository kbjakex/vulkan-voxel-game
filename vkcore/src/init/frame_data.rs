use crate::{Device, FrameData};

use anyhow::Result;
use erupt::vk;
use smallvec::SmallVec;

pub fn create_frame_data(device: &Device, frames_in_flight: u32) -> Result<SmallVec<[FrameData; 3]>> {
    let cmd_pool_info = vk::CommandPoolCreateInfoBuilder::new()
        .queue_family_index(device.queue.family_idx);

    let mut frames = SmallVec::new();

    let fence_info = vk::FenceCreateInfoBuilder::new().flags(vk::FenceCreateFlags::SIGNALED);
    let semaph_create_info = vk::SemaphoreCreateInfoBuilder::new();

    for _ in 0..frames_in_flight as usize {
        let cmd_pool = unsafe { device.create_command_pool(&cmd_pool_info, None) }.result()?;
        let cmd_buf_allocate_info = vk::CommandBufferAllocateInfoBuilder::new()
            .command_pool(cmd_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let cmd_bufs =
            unsafe { device.allocate_command_buffers(&cmd_buf_allocate_info) }.result()?;

        frames.push(FrameData {
            present_semaphore: unsafe { device.create_semaphore(&semaph_create_info, None) }
                .result()?,
            render_semaphore: unsafe { device.create_semaphore(&semaph_create_info, None) }
                .result()?,
            render_fence: unsafe { device.create_fence(&fence_info, None) }.result()?,
            command_pool: cmd_pool,
            main_command_buffer: cmd_bufs[0],
        })
    }

    Ok(frames)
}
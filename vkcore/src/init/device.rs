use std::{ffi::CStr, sync::Arc};

use anyhow::{bail, Context, Result};
use smallvec::SmallVec;

use crate::{debug, Device, Queue, Validation};

use erupt::{self, vk, DeviceLoader, InstanceLoader};

struct GraphicsDeviceDetails {
    queue_idx: u32,
    physical_device: vk::PhysicalDevice,
    properties: vk::PhysicalDeviceProperties,
    extensions: SmallVec<[*const i8; 1]>,
}

pub(crate) fn create_device(
    instance: &InstanceLoader,
    surface: vk::SurfaceKHR,
    validation: Validation,
) -> Result<Device> {
    let gpu_details = pick_suitable_gpu(instance, surface)?;

    let queue_info = &[vk::DeviceQueueCreateInfoBuilder::new()
        .queue_family_index(gpu_details.queue_idx)
        .queue_priorities(&[1.0])];

    let features = vk::PhysicalDeviceFeaturesBuilder::new().fill_mode_non_solid(true);

    let device_info = vk::DeviceCreateInfoBuilder::new()
        .queue_create_infos(queue_info)
        .enabled_features(&features)
        .enabled_extension_names(&gpu_details.extensions);

    let device = unsafe { DeviceLoader::new(instance, gpu_details.physical_device, &device_info) }?;

    debug!(validation, "Instantiation done!");

    let graphics_queue = Queue {
        handle: unsafe { device.get_device_queue(gpu_details.queue_idx, 0) },
        family_idx: gpu_details.queue_idx,
    };

    Ok(Device {
        logical: Arc::new(device),
        physical: gpu_details.physical_device,
        queue: graphics_queue,
        integrated: gpu_details.properties.device_type != vk::PhysicalDeviceType::DISCRETE_GPU
    })
}

fn pick_suitable_gpu(
    instance: &InstanceLoader,
    surface: vk::SurfaceKHR,
) -> Result<GraphicsDeviceDetails> {
    let opt = unsafe { instance.enumerate_physical_devices(None) }
        .map_err(|e| e)
        .context("enumerate_physical_devices")?
        .into_iter()
        .filter_map(|phys_device| get_gpu_details_if_suitable(phys_device, instance, surface))
        .max_by_key(rank_graphics_device);

    match opt {
        Some(details) => Ok(details),
        None => bail!("Could not find a suitable GPU! (Is one installed?)"),
    }
}

fn rank_graphics_device(graphics_device: &GraphicsDeviceDetails) -> i32 {
    match graphics_device.properties.device_type {
        vk::PhysicalDeviceType::DISCRETE_GPU => 2,
        vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
        _ => 0,
    }
}

fn get_gpu_details_if_suitable(
    phys_device: vk::PhysicalDevice,
    instance: &InstanceLoader,
    surface: vk::SurfaceKHR,
) -> Option<GraphicsDeviceDetails> {
    println!("Getting GPU details...");
    // 1. It has to support a) graphics and presentation and b) transfer. Might be in the same queue.
    // Noteworthy: graphics and compute imply transfer even if transfer bit is not set.
    let queue_family_props =
        unsafe { instance.get_physical_device_queue_family_properties(phys_device, None) };

    let queue_idx = match pick_queue_family(instance, phys_device, surface, &queue_family_props) {
        Some(idx) => idx,
        None => return None,
    };

    // 2. It has to support the desired features
    let properties = unsafe { instance.get_physical_device_properties(phys_device) };

    // 3. It has to support the desired extensions
    // (this allocation could be moved outside the function, whatever)
    let desired_device_extensions: SmallVec<_> = [
        vk::KHR_SWAPCHAIN_EXTENSION_NAME,
    ]
    .into();

    let supported_device_extensions =
        unsafe { instance.enumerate_device_extension_properties(phys_device, None, None) }.ok()?;

    let device_extensions_supported = desired_device_extensions.iter().all(|device_extension| {
        let device_extension = unsafe { CStr::from_ptr(*device_extension) };

        supported_device_extensions.iter().any(|properties| unsafe {
            CStr::from_ptr(properties.extension_name.as_ptr()) == device_extension
        })
    });

    if !device_extensions_supported {
        return None;
    }

    Some(GraphicsDeviceDetails {
        queue_idx,
        physical_device: phys_device,
        properties,
        extensions: desired_device_extensions,
    })
}

fn supports_present(
    i: usize,
    surface: vk::SurfaceKHR,
    device: vk::PhysicalDevice,
    instance: &InstanceLoader,
) -> bool {
    unsafe {
        instance
            .get_physical_device_surface_support_khr(device, i as u32, surface)
            .value
            .unwrap_or(false)
    }
}

fn pick_queue_family(
    instance: &InstanceLoader,
    physical_device: vk::PhysicalDevice,
    surface: vk::SurfaceKHR,
    queue_family_properties: &[vk::QueueFamilyProperties],
) -> Option<u32> {
    for (i, props) in queue_family_properties.iter().enumerate() {
        if !props
            .queue_flags
            .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE | vk::QueueFlags::TRANSFER)
            || !supports_present(i, surface, physical_device, instance)
        {
            continue;
        }

        return Some(i as _);
    }
    None
}

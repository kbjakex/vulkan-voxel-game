use erupt::{self, vk, InstanceLoader};

use anyhow::{Result, Context, bail};
use smallvec::SmallVec;

use crate::{swapchain::Swapchain, Device, Surface};

// Errors if:
//  1. No suitable surface format/present mode is found
//  2. vkGetPhysicalDeviceSurfaceCapabilitiesKHR fails because: OOM (CPU or GPU) or surface lost
//  3. vkCreateSwapchainKHR fails: OOM (CPU or GPU) or device/surface lost or something super strange going on
//  4. vkGetSwapchainImagesKHR fails: OOM (CPU or GPU)
// Basically should never fail on resizes unless things are messed up anyways
pub(crate) fn create_swapchain(
    instance: &InstanceLoader,
    device: &Device,
    surface: vk::SurfaceKHR,
    desired_present_mode: vk::PresentModeKHR,
    old_swapchain: vk::SwapchainKHR,
) -> Result<Swapchain> {
    let surface_format = select_surface_format(instance, device, surface)?;
    let present_mode = select_present_mode(instance, device, surface, desired_present_mode)?;

    let surface_capabilities =
        unsafe { instance.get_physical_device_surface_capabilities_khr(device.physical, surface) }
            .map_err(|e| e).context("get_physical_device_surface_capabilities_khr")?;

    let mut image_count = surface_capabilities.min_image_count + 1;
    if surface_capabilities.max_image_count > 0
        && image_count > surface_capabilities.max_image_count
    {
        image_count = surface_capabilities.max_image_count;
    }

    let swapchain_info = vk::SwapchainCreateInfoKHRBuilder::new()
        .surface(surface)
        .min_image_count(image_count)
        .image_format(surface_format.format)
        .image_color_space(surface_format.color_space)
        .image_extent(surface_capabilities.current_extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .pre_transform(surface_capabilities.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagBitsKHR::OPAQUE_KHR)
        .present_mode(present_mode)
        .clipped(true)
        .old_swapchain(old_swapchain);

    let swapchain = unsafe { device.logical.create_swapchain_khr(&swapchain_info, None) }
        .map_err(|e| e).context("create_swapchain_khr")?;

    let swapchain_images =
        unsafe { device.logical.get_swapchain_images_khr(swapchain, None) }
        .map_err(|e| e).context("get_swapchain_images_khr")?;
    let swapchain_images : SmallVec<[vk::Image;2]> = swapchain_images.into_iter().collect();

    let mut swapchain_image_views : SmallVec<[vk::ImageView; 2]> = SmallVec::new();
    for &handle in &swapchain_images {
        let view = match image_view_for_image(handle, device, surface_format.format) {
            Ok(view) => view,
            Err(e) => {
                bail!("Failed to create image view! Vulkan error: {}", e);
            },
        };
        swapchain_image_views.push(view);
    }
/*     images.push(Image {
        handle,
        view,
        format: surface_format.format,
        extent: surface_capabilities.current_extent,
        layers: 1,
        mem: None,
    });
 */
    Ok(Swapchain {
        handle: swapchain,
        surface: Surface {
            handle: surface,
            format: surface_format,
            extent: surface_capabilities.current_extent,
        },
        present_mode,
        images: swapchain_images,
        image_views: swapchain_image_views,
    })
}

fn image_view_for_image(image: vk::Image, gpu: &Device, format: vk::Format) -> Result<vk::ImageView> {
    let image_view_info = vk::ImageViewCreateInfoBuilder::new()
        .image(image)
        .view_type(vk::ImageViewType::_2D)
        .format(format)
        .components(vk::ComponentMapping {
            r: vk::ComponentSwizzle::IDENTITY,
            g: vk::ComponentSwizzle::IDENTITY,
            b: vk::ComponentSwizzle::IDENTITY,
            a: vk::ComponentSwizzle::IDENTITY,
        })
        .subresource_range(
            vk::ImageSubresourceRangeBuilder::new()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1)
                .build(),
        );
    unsafe { gpu.logical.create_image_view(&image_view_info, None) }
        .map_err(|e| e).context("create_image_view")
}

fn select_surface_format(
    instance: &InstanceLoader,
    device: &Device,
    surface: vk::SurfaceKHR,
) -> Result<vk::SurfaceFormatKHR> {
    let formats =
        unsafe { instance.get_physical_device_surface_formats_khr(device.physical, surface, None) }
            .map_err(|e| e).context("get_physical_device_surface_formats_khr")?;

    let res = formats
        .iter()
        .find(|surface_format| {
            println!("Found format {surface_format:?}");
            surface_format.format == vk::Format::B8G8R8A8_UNORM
                && surface_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR_KHR
        });
        //.or_else(|| formats.get(0));

    match res {
        Some(format) => {
            println!("{format:?}");
            Ok(*format)
        }
        None => bail!("select_surface_format: No surface formats found!")
    }
}

fn select_present_mode(
    instance: &InstanceLoader,
    device: &Device,
    surface: vk::SurfaceKHR,
    desired: vk::PresentModeKHR
) -> Result<vk::PresentModeKHR> {
    let present_modes = unsafe {
        instance.get_physical_device_surface_present_modes_khr(device.physical, surface, None)
    }
    .map_err(|e| e).context("get_physical_device_surface_present_modes_khr")?;

    Ok(*present_modes
        .iter()
        .find(|&present_mode| *present_mode == desired)
        .unwrap_or(&vk::PresentModeKHR::FIFO_KHR))
}

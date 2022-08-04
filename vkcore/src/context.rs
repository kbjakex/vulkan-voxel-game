use anyhow::{Context, Result};
use erupt::{vk, EntryLoader, InstanceLoader};
use smallvec::SmallVec;
use winit::window::Window;

use crate::{
    debug, pipeline::GraphicsPipelineBuilder, Device, FrameData, RenderPass,
    RenderPassDescriptor, Swapchain, Uploader, VkAllocator,
};

#[derive(Default)]
pub struct DeviceConfig<'a> {
    pub extensions: &'a [&'static str],
}

bitflags::bitflags! {
    pub struct DebugMsgSeverity : u32 {
        const INFO = vk::DebugUtilsMessageSeverityFlagBitsEXT::INFO_EXT.0 as _;
        const WARN = vk::DebugUtilsMessageSeverityFlagBitsEXT::WARNING_EXT.0 as _;
        const ERR = vk::DebugUtilsMessageSeverityFlagBitsEXT::ERROR_EXT.0 as _;
        const VERBOSE = vk::DebugUtilsMessageSeverityFlagBitsEXT::VERBOSE_EXT.0 as _;
    }
}

bitflags::bitflags! {
    pub struct DebugMsgType : u32 {
        const GENERAL = vk::DebugUtilsMessageTypeFlagBitsEXT::GENERAL_EXT.0 as _;
        const PERFORMANCE = vk::DebugUtilsMessageTypeFlagBitsEXT::PERFORMANCE_EXT.0 as _;
        const VALIDATION = vk::DebugUtilsMessageTypeFlagBitsEXT::VALIDATION_EXT.0 as _;
    }
}

#[derive(Clone, Copy)]
pub enum Validation {
    Disabled,
    EnabledWithDefaults,
    EnabledMaxVerbosity,
    Enabled(DebugMsgType, DebugMsgSeverity),
}

pub struct VkConfig<'a> {
    pub device: DeviceConfig<'a>,
    pub frames_in_flight: u32,
    pub present_mode: vk::PresentModeKHR,
    /// vk::make_api_version(0, 1, 2, 0) for 1.2
    pub vulkan_api_version: u32,
    pub validation: Validation,
}

impl<'a> Default for VkConfig<'a> {
    fn default() -> Self {
        Self {
            device: Default::default(),
            frames_in_flight: 2,
            present_mode: vk::PresentModeKHR::FIFO_KHR,
            vulkan_api_version: vk::make_api_version(0, 1, 2, 0),
            validation: Validation::Enabled(
                DebugMsgType::all(),
                DebugMsgSeverity::WARN | DebugMsgSeverity::ERR | DebugMsgSeverity::INFO,
            ),
        }
    }
}

pub struct VkContext {
    messenger: Option<vk::DebugUtilsMessengerEXT>,
    pub swapchain: Swapchain,
    pub device: Device,
    instance: InstanceLoader,
    _entry: EntryLoader,
    pub allocator: VkAllocator,
    pub uploader: Uploader,

    pub frames: SmallVec<[FrameData; 3]>,

    pub present_mode: vk::PresentModeKHR,
    pub frames_in_flight: u32,
}

impl<'a> VkContext {
    pub fn new(window: &Window, config: VkConfig) -> Result<Self> {
        let validation = config.validation;
        let entry = EntryLoader::new()?;
        debug!(
            validation,
            "Vulkan version {}.{}.{}",
            vk::api_version_major(entry.instance_version()),
            vk::api_version_minor(entry.instance_version()),
            vk::api_version_patch(entry.instance_version())
        );

        debug!(validation, "1/5 Creating instance");
        let instance = crate::init::instance::create_instance(&entry, window, &config)
            .context("create_instance")?;

        debug!(validation, "2/5 Creating debug messenger");
        let messenger = debug::get_debug_messenger_opt(&instance, validation)
            .context("get_debug_messenger_opt")?;

        debug!(validation, "3/5 Creating surface");
        let surface = unsafe { temp_helper::create_surface(&instance, window, None) }
            .map_err(|e| e)
            .context("create_surface")?;

        debug!(validation, "4/5 Creating device");
        let device = crate::init::device::create_device(&instance, surface, validation)
            .context("create_device")?;

        debug!(validation, "5/5 Creating swapchain");
        let swapchain = crate::init::swapchain::create_swapchain(
            &instance,
            &device,
            surface,
            config.present_mode,
            vk::SwapchainKHR::null(),
        )
        .context("create_swapchain")?;

        let mut allocator = VkAllocator::new(&device, &instance)?;

        let uploader = Uploader::new(&device, &mut allocator)?;

        let frames = crate::init::frame_data::create_frame_data(&device, config.frames_in_flight)?;

        Ok(VkContext {
            messenger,
            swapchain,
            device,
            instance,
            _entry: entry,
            allocator,
            uploader,
            frames,
            present_mode: config.present_mode,
            frames_in_flight: config.frames_in_flight,
        })
    }

    pub fn create_render_pass(&self, desc: RenderPassDescriptor) -> Result<RenderPass> {
        self.swapchain.create_render_pass(&self.device, desc)
    }

    pub fn graphics_pipeline_builder(&self) -> GraphicsPipelineBuilder {
        GraphicsPipelineBuilder::default(self)
    }

    pub fn recreate_swapchain(&mut self) -> Result<()> {
        unsafe {
            self.swapchain.destroy_self(&self.device);
        }

        self.swapchain = crate::init::swapchain::create_swapchain(
            &self.instance,
            &self.device,
            self.swapchain.surface.handle,
            self.present_mode,
            vk::SwapchainKHR::null(),
        )
        .context("create_swapchain")?;

        Ok(())
    }

    pub fn destroy_self(&mut self) -> Result<()> {
        self.uploader
            .destroy_self(&self.device, &mut self.allocator)?;

        unsafe {
            for frame in &self.frames {
                frame.destroy_self(&self.device);
            }
            self.swapchain.destroy_self(&self.device);
            self.instance
                .destroy_surface_khr(self.swapchain.surface.handle, None);
            self.device.destroy_device(None);

            if let Some(messenger) = self.messenger {
                self.instance
                    .destroy_debug_utils_messenger_ext(messenger, None);
            }

            self.instance.destroy_instance(None);
        }
        Ok(())
    }
}


// Copied and adapted from erupt/src/utils/surface.rs because Erupt's raw_window_handle dependency was outdated. 
// TODO!
pub(crate) mod temp_helper {
    use std::os::raw::c_char;

    use erupt::{InstanceLoader, utils::VulkanResult, extensions::khr_surface};
    use raw_window_handle::{HasRawWindowHandle, RawWindowHandle, HasRawDisplayHandle, RawDisplayHandle};

    pub unsafe fn create_surface(
        instance: &InstanceLoader,
        window_handle: &(impl HasRawWindowHandle + HasRawDisplayHandle),
        allocation_callbacks: Option<&erupt::vk1_0::AllocationCallbacks>,
    ) -> VulkanResult<khr_surface::SurfaceKHR> {
        match (window_handle.raw_window_handle(), window_handle.raw_display_handle()) {
            (RawWindowHandle::Wayland(handle), RawDisplayHandle::Wayland(dhandle)) => {
                use erupt::extensions::khr_wayland_surface;

                let create_info = khr_wayland_surface::WaylandSurfaceCreateInfoKHR {
                    display: dhandle.display,
                    surface: handle.surface,
                    ..Default::default()
                };

                instance.create_wayland_surface_khr(&create_info, allocation_callbacks)
            }
            (RawWindowHandle::Xlib(handle), RawDisplayHandle::Xlib(dhandle)) => {
                use erupt::extensions::khr_xlib_surface;

                let create_info = khr_xlib_surface::XlibSurfaceCreateInfoKHR {
                    dpy: dhandle.display as *mut _,
                    window: handle.window as _,
                    ..Default::default()
                };

                instance.create_xlib_surface_khr(&create_info, allocation_callbacks)
            }
            (RawWindowHandle::Xcb(handle), RawDisplayHandle::Xcb(dhandle)) => {
                use erupt::extensions::khr_xcb_surface;

                let create_info = khr_xcb_surface::XcbSurfaceCreateInfoKHR {
                    connection: dhandle.connection as *mut _,
                    window: handle.window,
                    ..Default::default()
                };

                instance.create_xcb_surface_khr(&create_info, allocation_callbacks)
            }
            #[cfg(any(target_os = "macos"))]
            RawWindowHandle::AppKit(handle) => {
                use erupt::{extensions::ext_metal_surface, vk1_0};
                use raw_window_metal::{appkit, Layer};

                let layer = match appkit::metal_layer_from_handle(handle) {
                    Layer::Existing(layer) | Layer::Allocated(layer) => layer as *mut _,
                    Layer::None => {
                        return VulkanResult::new_err(vk1_0::Result::ERROR_INITIALIZATION_FAILED)
                    }
                };

                let create_info = ext_metal_surface::MetalSurfaceCreateInfoEXT {
                    p_layer: layer,
                    ..Default::default()
                };

                instance.create_metal_surface_ext(&create_info, allocation_callbacks)
            }
            #[cfg(any(target_os = "ios"))]
            RawWindowHandle::UiKit(handle) => {
                use crate::{extensions::ext_metal_surface, vk1_0};
                use raw_window_metal::{uikit, Layer};

                let layer = match uikit::metal_layer_from_handle(handle) {
                    Layer::Existing(layer) | Layer::Allocated(layer) => layer as *mut _,
                    Layer::None => {
                        return VulkanResult::new_err(vk1_0::Result::ERROR_INITIALIZATION_FAILED)
                    }
                };

                let create_info = ext_metal_surface::MetalSurfaceCreateInfoEXT {
                    p_layer: layer,
                    ..Default::default()
                };

                instance.create_metal_surface_ext(&create_info, allocation_callbacks)
            }
            (RawWindowHandle::Win32(handle), RawDisplayHandle::Windows(_)) => {
                use erupt::extensions::khr_win32_surface;

                let create_info = khr_win32_surface::Win32SurfaceCreateInfoKHR {
                    hinstance: handle.hinstance,
                    hwnd: handle.hwnd,
                    ..Default::default()
                };

                instance.create_win32_surface_khr(&create_info, allocation_callbacks)
            }

            _ => VulkanResult::new_err(erupt::vk1_0::Result::ERROR_EXTENSION_NOT_PRESENT), // not supported
        }
    }

    pub fn enumerate_required_extensions(
        window_handle: &impl HasRawWindowHandle,
    ) -> VulkanResult<Vec<*const c_char>> {
        let extensions = match window_handle.raw_window_handle() {
            RawWindowHandle::Wayland(_) => vec![
                khr_surface::KHR_SURFACE_EXTENSION_NAME,
                erupt::extensions::khr_wayland_surface::KHR_WAYLAND_SURFACE_EXTENSION_NAME,
            ],
            RawWindowHandle::Xlib(_) => vec![
                khr_surface::KHR_SURFACE_EXTENSION_NAME,
                erupt::extensions::khr_xlib_surface::KHR_XLIB_SURFACE_EXTENSION_NAME,
            ],
            RawWindowHandle::Xcb(_) => vec![
                khr_surface::KHR_SURFACE_EXTENSION_NAME,
                erupt::extensions::khr_xcb_surface::KHR_XCB_SURFACE_EXTENSION_NAME,
            ],
            RawWindowHandle::AndroidNdk(_) => vec![
                khr_surface::KHR_SURFACE_EXTENSION_NAME,
                erupt::extensions::khr_android_surface::KHR_ANDROID_SURFACE_EXTENSION_NAME,
            ],
            RawWindowHandle::AppKit(_) | RawWindowHandle::UiKit(_) => vec![
                khr_surface::KHR_SURFACE_EXTENSION_NAME,
                erupt::extensions::ext_metal_surface::EXT_METAL_SURFACE_EXTENSION_NAME,
            ],
            RawWindowHandle::Win32(_) => vec![
                khr_surface::KHR_SURFACE_EXTENSION_NAME,
                erupt::extensions::khr_win32_surface::KHR_WIN32_SURFACE_EXTENSION_NAME,
            ],
            _ => return VulkanResult::new_err(erupt::vk1_0::Result::ERROR_EXTENSION_NOT_PRESENT), // not supported
        };
    
        VulkanResult::new_ok(extensions)
    }
    
}
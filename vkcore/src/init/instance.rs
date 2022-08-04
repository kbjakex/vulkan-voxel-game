use std::ffi::CString;

use erupt::{cstr, EntryLoader, vk, InstanceLoader, SmallVec};

use anyhow::{Result, Context};
use winit::{window::Window};

use crate::{VkConfig, Validation, temp_helper};

pub(crate) fn create_instance(entry: &EntryLoader, window: &Window, config: &VkConfig) -> Result<InstanceLoader> {
    let app_name = CString::new("AVulkanApp")?;
    let engine_name = CString::new("No Engine")?;

    let app_info = vk::ApplicationInfoBuilder::new()
        .application_name(&app_name)
        .application_version(vk::make_api_version(0, 1, 0, 0))
        .engine_name(&engine_name)
        .engine_version(vk::make_api_version(0, 1, 0, 0))
        .api_version(config.vulkan_api_version);

    let mut instance_extensions = temp_helper::enumerate_required_extensions(window)
        .map_err(|e| e)
        .context("enumerate_required_extensions")?;
    let mut instance_layers = SmallVec::new();

    if !matches!(config.validation, Validation::Disabled) {
        instance_extensions.push(vk::EXT_DEBUG_UTILS_EXTENSION_NAME);
        instance_layers.push(cstr!("VK_LAYER_KHRONOS_validation"));
    }

    let instance_info = vk::InstanceCreateInfoBuilder::new()
        .application_info(&app_info)
        .enabled_extension_names(&instance_extensions)
        .enabled_layer_names(&instance_layers);

    unsafe { InstanceLoader::new(entry, &instance_info) }.context("create_instance")
}


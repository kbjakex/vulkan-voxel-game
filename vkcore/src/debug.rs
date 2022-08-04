use std::ffi::{c_void, CStr};

use erupt::{vk, InstanceLoader};

use anyhow::{Result, bail};

use crate::Validation;

#[macro_export]
macro_rules! debug {
    ($validation:expr, $msg:literal $(,)?) => {
        if !matches!($validation, Validation::Disabled) {
            eprint!("[debug.rs]: ");
            eprintln!($msg);
        }
    };
    ($validation:expr, $err:expr $(,)?) => {
        if !matches!($validation, Validation::Disabled) {
            eprint!("[debug.rs]: ");
            eprintln!($err);
        }
    };
    ($validation:expr, $fmt:expr, $($arg:tt)*) => {
        if !matches!($validation, Validation::Disabled) {
            eprint!("[debug.rs]: ");
            eprintln!($fmt, $($arg)*);
        }
    };
}

fn extract_flags(validation: Validation) -> (vk::DebugUtilsMessageTypeFlagsEXT, vk::DebugUtilsMessageSeverityFlagsEXT) {
    match validation {
        Validation::Disabled => (vk::DebugUtilsMessageTypeFlagsEXT::empty(), vk::DebugUtilsMessageSeverityFlagsEXT::empty()), // can't happen
        Validation::EnabledWithDefaults => (
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL_EXT
            | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION_EXT
            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE_EXT,

            vk::DebugUtilsMessageSeverityFlagsEXT::INFO_EXT
            | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING_EXT
            | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR_EXT,
        ),
        Validation::EnabledMaxVerbosity => (
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL_EXT
            | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION_EXT
            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE_EXT,

            vk::DebugUtilsMessageSeverityFlagsEXT::INFO_EXT
            | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING_EXT
            | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR_EXT
            | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE_EXT
        ),
        Validation::Enabled(type_bits, severity_bits) => (
            vk::DebugUtilsMessageTypeFlagsEXT::from_bits(type_bits.bits()).unwrap(),
            vk::DebugUtilsMessageSeverityFlagsEXT::from_bits(severity_bits.bits()).unwrap()
        ),
    }
}

pub fn get_debug_messenger_opt(instance: &InstanceLoader, validation: Validation) -> Result<Option<vk::DebugUtilsMessengerEXT>> {
    let (type_flags, severity_flags) = extract_flags(validation);

    if type_flags.is_empty() || severity_flags.is_empty() {
        return Ok(None);
    }

    let messenger_info = vk::DebugUtilsMessengerCreateInfoEXTBuilder::new()
        .message_severity(severity_flags)
        .message_type(type_flags)
        .pfn_user_callback(Some(debug_callback));

    let res = unsafe {
        instance
            .create_debug_utils_messenger_ext(&messenger_info, None)
    };

    match res.map_err(|e| e) {
        Ok(messenger) => Ok(Some(messenger)),
        Err(err) => bail!("Failed to create messenger. Error code: {}", err),
    }
}

unsafe extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagBitsEXT,
    kind: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {

    let mut str = String::with_capacity(64);
    str += "[";
    if kind.contains(vk::DebugUtilsMessageTypeFlagsEXT::GENERAL_EXT) {
        str += "GENERAL";
    }
    if kind.contains(vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION_EXT) {
        if str.len() > 1 {
            str += "/";
        }
        str += "VALIDATION";
    }
    if kind.contains(vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE_EXT) {
        if str.len() > 1 {
            str += "/";
        }
        str += "PERF";
    }

    str += " ";

    let severity = severity.bitmask();
    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::INFO_EXT) {
        str += "INFO";
    }
    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING_EXT) {
        if str.len() > 1 {
            str += "/";
        }
        str += "WARN";
    }
    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR_EXT) {
        if str.len() > 1 {
            str += "/";
        }
        str += "ERROR";
    }
    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE_EXT) {
        if str.len() > 1 {
            str += "/";
        }
        str += "VERBOSE";
    }
    str += "]";

    eprintln!(
        "[debug.rs]: {} {}",
        str,
        CStr::from_ptr((*p_callback_data).p_message).to_string_lossy()
    );

    vk::FALSE
}
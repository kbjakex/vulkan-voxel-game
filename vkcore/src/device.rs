use std::{sync::Arc, ops::Deref};

use erupt::vk;

pub struct Device {
    pub logical: Arc<erupt::DeviceLoader>,
    pub physical: vk::PhysicalDevice,
    pub integrated: bool,

    pub queue: Queue,
}

impl Deref for Device {
    type Target = erupt::DeviceLoader;

    fn deref(&self) -> &Self::Target {
        &*self.logical
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Queue {
    pub(crate) handle: vk::Queue, // deref to get access
    pub family_idx: u32,
}

impl Deref for Queue {
    type Target = vk::Queue;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

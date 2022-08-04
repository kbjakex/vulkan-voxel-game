mod debug;
mod init;

pub mod context;
pub mod device;
pub mod render_pass;
pub mod swapchain;
pub mod pipeline;
pub mod allocator;
pub mod uploader;

pub use context::*;
pub use device::*;
pub use render_pass::*;
pub use swapchain::*;
pub use allocator::*;
pub use uploader::*;

#[no_mangle]
pub static NvOptimusEnablement: i32 = 1;

#[no_mangle]
pub static AmdPowerXpressRequestHighPerformance: i32 = 1;

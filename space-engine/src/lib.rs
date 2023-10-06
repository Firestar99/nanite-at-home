extern crate core;

/// for macro use
pub use paste;

pub use vulkan::window::event_loop::event_loop_init;

pub mod vulkan;
pub mod application_config;
pub mod space;

pub(crate) mod shader;

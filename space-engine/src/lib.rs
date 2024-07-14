extern crate core;

/// for macro use
pub use paste;
pub use window::event_loop::event_loop_init;

pub mod application_config;
pub mod device;
pub mod pipeline_cache;
pub mod renderer;
pub(crate) mod shader;
pub mod window;

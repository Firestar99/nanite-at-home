#![no_std]
// otherwise you won't see any warnings
#![deny(warnings)]

#[cfg(test)]
extern crate alloc;

pub mod buffer_content;
pub mod descriptor;
pub mod frame_in_flight;
pub mod shader_type;

pub use bytemuck;
pub use bytemuck_derive;
pub use spirv_std;
pub use spirv_std::{spirv, Image};
pub use static_assertions;

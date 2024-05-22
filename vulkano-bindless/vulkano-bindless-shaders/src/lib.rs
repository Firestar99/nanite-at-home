#![no_std]

#[cfg(test)]
extern crate alloc;

pub mod desc_buffer;
pub mod descriptor;
pub mod frame_in_flight;
pub mod shader_type;

pub use bytemuck;
pub use bytemuck_derive;
pub use spirv_std;
pub use spirv_std::{spirv, Image};

#![no_std]

#[cfg(test)]
extern crate alloc;

pub mod descriptor;
pub mod frame_in_flight;
pub mod param;
pub mod shader_type;

extern crate spirv_std as macros;
pub use crate::macros::spirv;
pub use crate::macros::Image;
pub use spirv_std;

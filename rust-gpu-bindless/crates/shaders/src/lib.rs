#![no_std]
// otherwise you won't see any warnings
#![deny(warnings)]

#[cfg(test)]
extern crate alloc;

pub mod buffer_content;
pub mod descriptor;
pub mod shader;
pub mod shader_type;
pub mod utils;

pub use spirv_std;
pub use spirv_std::{Image, spirv};

pub mod __private {
	pub use rust_gpu_bindless_buffer_content::__private::*;
}

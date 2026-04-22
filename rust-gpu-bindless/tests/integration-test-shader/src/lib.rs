#![no_std]

#[cfg(not(target_arch = "spirv"))]
extern crate alloc;
extern crate core;
#[cfg(not(target_arch = "spirv"))]
extern crate std;

pub mod buffer_barriers;
pub mod color;
pub mod simple_compute;
pub mod triangle;

#![no_std]
// allows `debug_printf!()` to be used in #[gpu_only] context
#![cfg_attr(target_arch = "spirv", feature(asm_experimental_arch))]
// otherwise you won't see any warnings
#![deny(warnings)]

#[cfg(not(target_arch = "spirv"))]
extern crate alloc;
extern crate core;
#[cfg(not(target_arch = "spirv"))]
extern crate std;

pub mod buffer_barriers;
pub mod color;
pub mod simple_compute;
pub mod triangle;

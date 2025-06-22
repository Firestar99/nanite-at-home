#![cfg_attr(target_arch = "spirv", no_std)]
// allows `debug_printf!()` to be used in #[gpu_only] context
#![cfg_attr(target_arch = "spirv", feature(asm_experimental_arch))]
// otherwise you won't see any warnings
#![deny(warnings)]

extern crate core;

pub mod material;
pub mod renderer;
pub mod screen_space_trace;
// pub mod screen_space_old;
pub mod utils;

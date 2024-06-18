#![cfg_attr(target_arch = "spirv", no_std)]
// otherwise you won't see any warnings
#![deny(warnings)]

extern crate core;

pub mod space;

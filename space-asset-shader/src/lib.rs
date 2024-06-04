#![cfg_attr(target_arch = "spirv", no_std)]
// otherwise you won't see any warnings
#![deny(warnings)]

pub mod meshlet;
pub mod opaque_model;

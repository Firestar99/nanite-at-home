#![cfg_attr(target_arch = "spirv", no_std)]
// otherwise you won't see any warnings
#![cfg_attr(not(target_arch = "spirv"), deny(warnings))]
#![allow(unused_imports)]

pub mod affine_transform;
pub mod material;
pub mod meshlet;

pub use space_asset_disk_shader::*;

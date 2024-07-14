#![cfg_attr(target_arch = "spirv", no_std)]
// otherwise you won't see any warnings
#![cfg_attr(not(any(feature = "disk", feature = "runtime")), deny(warnings))]

pub mod image;
pub mod material;
pub mod meshlet;
pub mod uploader;

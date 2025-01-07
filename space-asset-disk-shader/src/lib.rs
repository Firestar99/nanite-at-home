#![cfg_attr(not(feature = "disk"), no_std)]
// otherwise you won't see any warnings
#![cfg_attr(target_arch = "spirv", deny(warnings))]

pub mod material;
pub mod meshlet;
pub mod range;
pub mod shape;

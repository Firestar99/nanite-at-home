#![cfg_attr(target_arch = "spirv", no_std)]

// otherwise you won't see any warnings
#![deny(warnings)]

// at least one shader or this import is required, otherwise compile will fail
#[allow(unused_imports)]
use spirv_std::spirv;

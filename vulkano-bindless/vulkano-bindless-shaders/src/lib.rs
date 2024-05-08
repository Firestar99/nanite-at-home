#![no_std]

pub mod descriptor;
pub mod frame_in_flight;

pub extern crate spirv_std as macros;
pub use crate::macros::spirv;
pub use crate::macros::Image;
pub use spirv_std;

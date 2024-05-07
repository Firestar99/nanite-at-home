#![no_std]

pub mod descriptor;
pub mod frame_in_flight;

#[macro_use]
pub extern crate spirv_std as macros;
pub use crate::macros::Image;

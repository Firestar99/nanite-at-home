use bytemuck_derive::AnyBitPattern;

use crate::space::renderer::camera::Camera;

#[derive(Copy, Clone, AnyBitPattern)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct FrameData {
	pub camera: Camera,
}

use crate::space::renderer::camera::Camera;
use bytemuck_derive::AnyBitPattern;

#[derive(Copy, Clone, AnyBitPattern)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct FrameData {
	pub camera: Camera,
}

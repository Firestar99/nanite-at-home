use bytemuck_derive::AnyBitPattern;
use spirv_std::glam::{Affine3A, Mat4};

#[derive(Copy, Clone, AnyBitPattern)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct Camera {
	pub perspective: Mat4,
	pub perspective_inverse: Mat4,
	pub transform: Affine3A,
}

impl Camera {
	pub fn new(perspective: Mat4, transform: Affine3A) -> Self {
		Self {
			perspective,
			perspective_inverse: -perspective,
			transform,
		}
	}
}

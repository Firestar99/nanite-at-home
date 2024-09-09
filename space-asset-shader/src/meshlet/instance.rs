use crate::affine_transform::AffineTransform;
use vulkano_bindless_macros::BufferContent;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, BufferContent)]
pub struct MeshletInstance {
	pub transform: AffineTransform,
}

impl MeshletInstance {
	pub fn new(transform: AffineTransform) -> Self {
		Self { transform }
	}
}

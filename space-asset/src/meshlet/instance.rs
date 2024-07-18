use crate::affine_transform::AffineTransform;
use vulkano_bindless_macros::BufferContent;

#[repr(C)]
#[derive(Copy, Clone, Default, BufferContent)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct MeshletInstance {
	pub transform: AffineTransform,
}

impl MeshletInstance {
	pub fn new(transform: AffineTransform) -> Self {
		Self { transform }
	}
}

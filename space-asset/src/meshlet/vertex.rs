use core::mem;
use glam::Vec3;
use static_assertions::const_assert_eq;
use vulkano_bindless_macros::BufferContent;

#[repr(C)]
#[derive(Copy, Clone, BufferContent)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct MeshletVertex {
	pub position: [f32; 3],
}
const_assert_eq!(mem::size_of::<MeshletVertex>(), 3 * 4);

impl MeshletVertex {
	pub fn new(position: Vec3) -> Self {
		Self {
			position: position.to_array(),
		}
	}

	pub fn position(&self) -> Vec3 {
		Vec3::from(self.position)
	}
}

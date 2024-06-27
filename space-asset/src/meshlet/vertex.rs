use bytemuck_derive::{Pod, Zeroable};
use core::mem;
use glam::Vec3;
use static_assertions::const_assert_eq;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Zeroable, Pod)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct MeshletDrawVertex {
	pub position: [f32; 3],
}
const_assert_eq!(mem::size_of::<MeshletDrawVertex>(), 3 * 4);

impl MeshletDrawVertex {
	pub fn new(position: Vec3) -> Self {
		Self {
			position: position.to_array(),
		}
	}

	pub fn position(&self) -> Vec3 {
		Vec3::from(self.position)
	}
}

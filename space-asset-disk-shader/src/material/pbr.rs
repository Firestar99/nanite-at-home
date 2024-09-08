use glam::{Vec2, Vec3};
use vulkano_bindless_macros::{assert_transfer_size, BufferContent};

#[repr(C)]
#[derive(Copy, Clone, Debug, BufferContent)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct PbrVertex {
	pub normals: Vec3,
	pub tex_coords: Vec2,
}
assert_transfer_size!(PbrVertex, 5 * 4);

use glam::{Vec2, Vec3, Vec4};
use rust_gpu_bindless_macros::{assert_transfer_size, BufferStructPlain};

#[repr(C)]
#[derive(Copy, Clone, Debug, BufferStructPlain)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct PbrVertex {
	pub tangent: Vec4,
	pub normal: Vec3,
	pub tex_coord: Vec2,
}
assert_transfer_size!(PbrVertex, 9 * 4);

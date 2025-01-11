use crate::meshlet::lod_level_bitmask::LodLevelBitmask;
use crate::meshlet::offset::MeshletOffset;
use crate::shape::sphere::Sphere;
use rust_gpu_bindless_macros::{assert_transfer_size, BufferStructPlain};

#[repr(C)]
#[derive(Copy, Clone, Debug, BufferStructPlain)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct MeshletData {
	pub draw_vertex_offset: MeshletOffset,
	pub triangle_offset: MeshletOffset,
	pub bounds: Sphere,
	pub parent_bounds: Sphere,
	pub error: f32,
	pub parent_error: f32,
	pub lod_level_bitmask: LodLevelBitmask,
	pub _pad: [u32; 1],
}
assert_transfer_size!(MeshletData, 16 * 4);

impl AsRef<MeshletData> for MeshletData {
	fn as_ref(&self) -> &MeshletData {
		self
	}
}

use crate::meshlet::offset::MeshletOffset;
use vulkano_bindless_macros::{assert_transfer_size, BufferContentPlain};

#[repr(C)]
#[derive(Copy, Clone, Debug, BufferContentPlain)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct MeshletData {
	pub draw_vertex_offset: MeshletOffset,
	pub triangle_offset: MeshletOffset,
}
assert_transfer_size!(MeshletData, 2 * 4);

impl AsRef<MeshletData> for MeshletData {
	fn as_ref(&self) -> &MeshletData {
		self
	}
}

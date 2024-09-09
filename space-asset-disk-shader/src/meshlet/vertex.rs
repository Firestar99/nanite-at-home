use glam::Vec3;
use vulkano_bindless_macros::{assert_transfer_size, BufferContentPlain};

#[repr(transparent)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, BufferContentPlain)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct MaterialVertexId(pub u32);
assert_transfer_size!(MaterialVertexId, 4);

#[repr(C)]
#[derive(Copy, Clone, Debug, BufferContentPlain)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct DrawVertex {
	pub position: Vec3,
	pub material_vertex_id: MaterialVertexId,
}
assert_transfer_size!(DrawVertex, 4 * 4);

use glam::Vec3;
use vulkano_bindless_macros::{assert_transfer_size, BufferContent};

#[repr(transparent)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, BufferContent)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct MaterialVertexId(pub u32);
assert_transfer_size!(MaterialVertexId, 4);

#[repr(C)]
#[derive(Copy, Clone, Debug, BufferContent)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct DrawVertex {
	pub position: Vec3,
	pub material_vertex_id: MaterialVertexId,
}
assert_transfer_size!(DrawVertex, 4 * 4);

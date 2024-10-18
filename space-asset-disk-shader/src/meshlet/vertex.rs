use core::ops::Deref;
use glam::Vec3;
use rust_gpu_bindless_macros::{assert_transfer_size, BufferContentPlain};

#[repr(transparent)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, BufferContentPlain)]
#[cfg_attr(feature = "disk", derive(bytemuck_derive::Zeroable, bytemuck_derive::Pod))]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct MaterialVertexId(pub u32);
assert_transfer_size!(MaterialVertexId, 4);

impl Deref for MaterialVertexId {
	type Target = u32;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[repr(C)]
#[derive(Copy, Clone, Debug, BufferContentPlain)]
#[cfg_attr(feature = "disk", derive(bytemuck_derive::Zeroable, bytemuck_derive::Pod))]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct DrawVertex {
	pub position: Vec3,
	pub material_vertex_id: MaterialVertexId,
}
assert_transfer_size!(DrawVertex, 4 * 4);

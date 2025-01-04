use crate::renderer::meshlet::meshlet_select::MESHLET_SELECT_WG_SIZE;
use rust_gpu_bindless_macros::BufferStructPlain;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, BufferStructPlain)]
pub struct MeshletInstance {
	pub instance_id: u32,
	pub mesh_id: u32,
	pub meshlet_id: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, BufferStructPlain)]
pub struct MeshletGroupInstance {
	pub instance_id: u32,
	pub mesh_id: u32,
	pub meshlet_id: u32,
	pub meshlet_cnt: u32,
}

impl MeshletGroupInstance {
	pub const MAX_MESHLET_CNT: u32 = MESHLET_SELECT_WG_SIZE;
}

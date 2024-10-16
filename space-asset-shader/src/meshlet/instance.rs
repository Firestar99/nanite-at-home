use crate::affine_transform::AffineTransform;
use space_asset_disk_shader::range::RangeU32;
use vulkano_bindless_macros::BufferContent;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, BufferContent)]
pub struct MeshInstance {
	pub transform: AffineTransform,
	pub mesh_ids: RangeU32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, BufferContent)]
pub struct MeshletInstance {
	pub instance_id: u32,
	pub mesh_id: u32,
	pub meshlet_id: u32,
}

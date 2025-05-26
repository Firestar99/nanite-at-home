use crate::affine_transform::AffineTransform;
use rust_gpu_bindless_macros::BufferStructPlain;
use space_asset_disk_shader::range::RangeU32;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, BufferStructPlain)]
pub struct MeshInstance {
	pub world_from_local: AffineTransform,
	pub mesh_ids: RangeU32,
}

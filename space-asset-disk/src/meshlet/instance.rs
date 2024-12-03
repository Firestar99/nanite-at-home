use glam::Affine3A;
use rust_gpu_bindless_macros::BufferStructPlain;
use space_asset_disk_shader::range::RangeU32;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, BufferStructPlain, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct MeshletInstanceDisk {
	pub transform: Affine3A,
	pub mesh_ids: RangeU32,
}

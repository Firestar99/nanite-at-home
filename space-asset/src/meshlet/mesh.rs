use vulkano_bindless::descriptor::reference::DescStructRef;
use vulkano_bindless::descriptor::{Buffer, DescRef, RCDesc};

pub use space_asset_shader::meshlet::mesh::*;

pub struct MeshletCpuMesh<R: DescRef + DescStructRef + Copy + 'static> {
	pub mesh: RCDesc<Buffer<MeshletMesh<R>>>,
	pub num_meshlets: u32,
}

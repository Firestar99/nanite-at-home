pub use space_asset_shader::meshlet::mesh::*;
use vulkano_bindless::desc_buffer::DescStruct;
use vulkano_bindless::descriptor::{Buffer, DescRef, RCDesc};

pub struct MeshletCpuMesh<R: DescRef + Copy + 'static>
// FIXME why???
where
	MeshletMesh<R>: DescStruct,
{
	pub mesh: RCDesc<Buffer<MeshletMesh<R>>>,
	pub num_meshlets: u32,
}

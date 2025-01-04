use crate::material::pbr::PbrMaterial;
use crate::meshlet::instance::MeshInstance;
use crate::meshlet::mesh::MeshletMesh;
use vulkano_bindless_macros::BufferContent;
use vulkano_bindless_shaders::descriptor::{Buffer, Desc, DescRef, Strong};

#[derive(Copy, Clone, Debug, BufferContent)]
pub struct MeshletScene<R: DescRef> {
	pub meshes: Desc<R, Buffer<[MeshletMesh<Strong>]>>,
	pub instances: Desc<R, Buffer<[MeshInstance]>>,
}

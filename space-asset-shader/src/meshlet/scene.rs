use crate::material::pbr::PbrMaterial;
use crate::meshlet::instance::MeshInstance;
use crate::meshlet::mesh::MeshletMesh;
use rust_gpu_bindless_macros::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::{Buffer, Desc, DescRef, Strong};

#[repr(C)]
#[derive(Copy, Clone, Debug, BufferStruct)]
pub struct MeshletScene<R: DescRef> {
	pub meshes: Desc<R, Buffer<[MeshletMesh<Strong>]>>,
	pub instances: Desc<R, Buffer<[MeshInstance]>>,
}
